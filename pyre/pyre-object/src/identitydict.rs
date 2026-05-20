//! `pypy/objspace/std/identitydict.py` port — strategy for custom
//! instances which compare by identity (the default unless you
//! override `__hash__`, `__eq__` or `__cmp__`).
//!
//! Selected by `EmptyDictStrategy.switch_to_correct_strategy` per
//! `dictmultiobject.py:725-730 switch_to_identity_strategy` when the
//! key's type satisfies `W_TypeObject.compares_by_identity()`
//! (`typeobject.py:353-371`).  Storage shape matches
//! `ObjectDictStrategy` — the distinction is in the comparison
//! function: raw PyObjectRef equality (`==`) instead of
//! `dict_keys_equal` (`space.eq_w` + `space.hash_w`).

#![allow(unsafe_op_in_unsafe_fn)]

use crate::dictstrategy::{DictStrategy, OBJECT_DICT_STRATEGY};
use crate::pyobject::PyObjectRef;

/// `identitydict.py:12-83 IdentityDictStrategy`.
///
/// ```python
/// class IdentityDictStrategy(AbstractTypedStrategy, DictStrategy):
///     erase, unerase = rerased.new_erasing_pair("identitydict")
///
///     def wrap(self, unwrapped):
///         return unwrapped
///     def unwrap(self, wrapped):
///         return wrapped
///
///     def get_empty_storage(self):
///         d = {}
///         mark_dict_non_null(d)
///         return self.erase(d)
///
///     def is_correct_type(self, w_obj):
///         w_type = self.space.type(w_obj)
///         return w_type.compares_by_identity()
///
///     def _never_equal_to(self, w_lookup_type):
///         return False
///
///     def w_keys(self, w_dict):
///         return self.space.newlist(self.unerase(w_dict.dstorage).keys())
/// ```
pub struct IdentityDictStrategy;

/// `pypy/objspace/std/identitydict.py:12 IdentityDictStrategy`
/// singleton — matches PyPy's `space.fromcache(IdentityDictStrategy)`.
pub static IDENTITY_DICT_STRATEGY: IdentityDictStrategy = IdentityDictStrategy;

impl IdentityDictStrategy {
    /// `identitydict.py:36-37 IdentityDictStrategy.is_correct_type` —
    /// `self.space.type(w_obj).compares_by_identity()`.  Dispatch
    /// through the `dict_eq_hook::COMPARES_BY_IDENTITY_HOOK`
    /// trampoline (pyre-interpreter installs the MRO walker).
    #[inline]
    unsafe fn is_correct_type(w_key: PyObjectRef) -> bool {
        let w_type = (*w_key).w_class as PyObjectRef;
        if w_type.is_null() {
            return false;
        }
        matches!(
            crate::dict_eq_hook::try_compares_by_identity(w_type),
            Some(true)
        )
    }

    /// `dictmultiobject.py:1143-1150 AbstractTypedStrategy.switch_to_object_strategy`
    /// instantiation for IdentityDictStrategy — `wrap` is identity
    /// (`:26-27`), so the migration just relabels the storage as
    /// ObjectDictStrategy without rewrapping keys.  Pyre still
    /// reallocates the storage box to match the PyPy
    /// `set_strategy + dstorage = strategy.erase(d_new)` shape.
    ///
    /// # Safety
    /// `w_dict` must point at a valid `W_DictObject` on
    /// [`IDENTITY_DICT_STRATEGY`].
    unsafe fn switch_to_object_strategy(&self, w_dict: PyObjectRef) {
        let dict = &mut *(w_dict as *mut crate::dictmultiobject::W_DictObject);
        let old = Box::from_raw(dict.dstorage as *mut Vec<(PyObjectRef, PyObjectRef)>);
        // `wrap` is identity so the entries port across unchanged.
        let new_box = Box::new(old.into_iter().collect::<Vec<_>>());
        dict.dstorage = Box::into_raw(new_box) as *mut u8;
        dict.dstrategy = &OBJECT_DICT_STRATEGY;
    }
}

impl DictStrategy for IdentityDictStrategy {
    /// `identitydict.py:67-70 get_empty_storage` — erased `{}` with
    /// non-null hint.  Pyre stores `Vec<(PyObjectRef, PyObjectRef)>`
    /// matching ObjectDictStrategy's shape.
    fn get_empty_storage(&self) -> *mut u8 {
        let v: Box<Vec<(PyObjectRef, PyObjectRef)>> = Box::new(Vec::new());
        Box::into_raw(v) as *mut u8
    }

    /// `dictmultiobject.py:1095-1103 AbstractTypedStrategy.getitem` —
    /// linear scan with raw PyObjectRef `==`.
    unsafe fn getitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> Option<PyObjectRef> {
        if Self::is_correct_type(w_key) {
            let entries = crate::dictmultiobject::w_dict_object_storage(w_dict);
            for &(k, v) in entries {
                if k == w_key {
                    return Some(v);
                }
            }
            return None;
        }
        // `identitydict.py:40-41 _never_equal_to` → always False, so
        // mismatched keys always promote and retry.
        self.switch_to_object_strategy(w_dict);
        crate::dictmultiobject::w_dict_lookup(w_dict, w_key)
    }

    /// `dictmultiobject.py:1061-1067 setitem` — linear scan with raw
    /// PyObjectRef `==`; on mismatch, promote to Object.
    unsafe fn setitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef, w_value: PyObjectRef) {
        if Self::is_correct_type(w_key) {
            let dict = &mut *(w_dict as *mut crate::dictmultiobject::W_DictObject);
            let entries = &mut *(dict.dstorage as *mut Vec<(PyObjectRef, PyObjectRef)>);
            for entry in entries.iter_mut() {
                if entry.0 == w_key {
                    entry.1 = w_value;
                    crate::gc_hook::try_gc_write_barrier(w_dict as *mut u8);
                    return;
                }
            }
            entries.push((w_key, w_value));
            dict.len += 1;
            crate::gc_hook::try_gc_write_barrier(w_dict as *mut u8);
            return;
        }
        self.switch_to_object_strategy(w_dict);
        crate::dictmultiobject::w_dict_store(w_dict, w_key, w_value);
    }

    /// `dictmultiobject.py:1069-1071 setitem_str` — IdentityDictStrategy
    /// promotes to Object on str setitem_str (str has its own
    /// non-identity `__eq__` / `__hash__`, so OVERRIDES_EQ_CMP_OR_HASH).
    unsafe fn setitem_str(&self, w_dict: PyObjectRef, key: &str, w_value: PyObjectRef) {
        self.switch_to_object_strategy(w_dict);
        crate::dictmultiobject::w_dict_setitem_str(w_dict, key, w_value);
    }

    /// `dictmultiobject.py:1081-1087 delitem` — raw PyObjectRef `==`
    /// removal; on mismatch, promote to Object.
    unsafe fn delitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> bool {
        if Self::is_correct_type(w_key) {
            let dict = &mut *(w_dict as *mut crate::dictmultiobject::W_DictObject);
            let entries = &mut *(dict.dstorage as *mut Vec<(PyObjectRef, PyObjectRef)>);
            let before = entries.len();
            entries.retain(|&(k, _)| k != w_key);
            if entries.len() != before {
                dict.len = entries.len();
                return true;
            }
            return false;
        }
        self.switch_to_object_strategy(w_dict);
        crate::dictmultiobject::w_dict_delitem(w_dict, w_key)
    }

    unsafe fn length(&self, w_dict: PyObjectRef) -> usize {
        crate::dictmultiobject::w_dict_object_storage(w_dict).len()
    }

    unsafe fn w_keys(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        crate::dictmultiobject::w_dict_object_storage(w_dict)
            .iter()
            .map(|&(k, _)| k)
            .collect()
    }

    unsafe fn values(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        crate::dictmultiobject::w_dict_object_storage(w_dict)
            .iter()
            .map(|&(_, v)| v)
            .collect()
    }

    unsafe fn items(&self, w_dict: PyObjectRef) -> Vec<(PyObjectRef, PyObjectRef)> {
        crate::dictmultiobject::w_dict_object_storage(w_dict).clone()
    }

    unsafe fn clear(&self, w_dict: PyObjectRef) {
        let dict = &mut *(w_dict as *mut crate::dictmultiobject::W_DictObject);
        let entries = &mut *(dict.dstorage as *mut Vec<(PyObjectRef, PyObjectRef)>);
        entries.clear();
        dict.len = 0;
    }
}
