//! `pypy/objspace/std/identitydict.py` port — strategy for custom
//! instances which compare by identity (the default unless you
//! override `__hash__`, `__eq__` or `__cmp__`).
//!
//! Selected by `EmptyDictStrategy.switch_to_correct_strategy` per
//! `dictmultiobject.py:736-754`.  Storage is a normal RPython dict
//! since identity is already the default semantics for object-keyed
//! hashing in CPython / PyPy.
//!
//! Skeleton: stub implementation pending the `W_DictObject.dstorage`
//! migration (Phase C-3).  Methods route through pyre's existing
//! W_DictObject Vec storage for now so the strategy can be switched
//! on without disturbing call sites.

#![allow(unsafe_op_in_unsafe_fn)]

use crate::dictstrategy::DictStrategy;
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

impl DictStrategy for IdentityDictStrategy {
    fn get_empty_storage(&self) -> *mut u8 {
        // `identitydict.py:67-70`: erased `{}` with non-null hint.
        let v: Box<Vec<(PyObjectRef, PyObjectRef)>> = Box::new(Vec::new());
        Box::into_raw(v) as *mut u8
    }

    unsafe fn getitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> Option<PyObjectRef> {
        crate::dictmultiobject::w_dict_lookup(w_dict, w_key)
    }

    unsafe fn setitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef, w_value: PyObjectRef) {
        crate::dictmultiobject::w_dict_store(w_dict, w_key, w_value);
    }

    unsafe fn delitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> bool {
        crate::dictmultiobject::w_dict_delitem(w_dict, w_key)
    }

    unsafe fn length(&self, w_dict: PyObjectRef) -> usize {
        crate::dictmultiobject::w_dict_len(w_dict)
    }

    unsafe fn w_keys(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        crate::dictmultiobject::w_dict_items(w_dict)
            .into_iter()
            .map(|(k, _)| k)
            .collect()
    }

    unsafe fn values(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        crate::dictmultiobject::w_dict_items(w_dict)
            .into_iter()
            .map(|(_, v)| v)
            .collect()
    }

    unsafe fn items(&self, w_dict: PyObjectRef) -> Vec<(PyObjectRef, PyObjectRef)> {
        crate::dictmultiobject::w_dict_items(w_dict)
    }

    unsafe fn clear(&self, w_dict: PyObjectRef) {
        crate::dictmultiobject::w_dict_clear(w_dict);
    }
}
