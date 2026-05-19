//! `pypy/objspace/std/kwargsdict.py` port — dict implementation
//! specialized for keyword argument dicts.
//!
//! Based on two parallel lists `(keys_w, values_w)` of `PyObjectRef`.
//! Optimized for the common `**kwargs` shape: a small number of
//! distinct string keys with O(n) linear-scan lookup that the JIT
//! constant-folds when the dict size and lookup key are both
//! constant.
//!
//! Skeleton port — concrete `EmptyKwargsDictStrategy` swap from
//! `EmptyDictStrategy.switch_to_unicode_strategy` lives in the same
//! file per `kwargsdict.py:13-22`.

#![allow(unsafe_op_in_unsafe_fn)]

use crate::dictstrategy::DictStrategy;
use crate::pyobject::PyObjectRef;

/// `kwargsdict.py:25-198 KwargsDictStrategy`.
///
/// ```python
/// class KwargsDictStrategy(DictStrategy):
///     erase, unerase = rerased.new_erasing_pair("kwargsdict")
///
///     def get_empty_storage(self):
///         d = ([], [])
///         return self.erase(d)
///
///     def is_correct_type(self, w_obj):
///         space = self.space
///         return space.is_w(space.type(w_obj), space.w_text)
///
///     def setitem(self, w_dict, w_key, w_value):
///         if self.is_correct_type(w_key):
///             self.setitem_correct(w_dict, w_key, w_value)
///             return
///         else:
///             self.switch_to_object_strategy(w_dict)
///             w_dict.setitem(w_key, w_value)
/// ```
///
/// Two-list backing chosen because:
/// - Function-call sites always create small kwarg dicts.
/// - The JIT can fold the entire lookup loop when both size and key
///   are constant via `jit.look_inside_iff`.
/// - At size ≥ 16 entries (`kwargsdict.py:62`) the strategy
///   auto-promotes to `UnicodeDictStrategy` to avoid degenerate O(n).
pub struct KwargsDictStrategy;

/// `pypy/objspace/std/kwargsdict.py:25 KwargsDictStrategy`
/// singleton — matches PyPy's `space.fromcache(KwargsDictStrategy)`.
pub static KWARGS_DICT_STRATEGY: KwargsDictStrategy = KwargsDictStrategy;

impl DictStrategy for KwargsDictStrategy {
    fn get_empty_storage(&self) -> *mut u8 {
        // `kwargsdict.py:30-32`: erased `([], [])` parallel arrays.
        let v: Box<(Vec<PyObjectRef>, Vec<PyObjectRef>)> = Box::new((Vec::new(), Vec::new()));
        Box::into_raw(v) as *mut u8
    }

    unsafe fn getitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> Option<PyObjectRef> {
        // `kwargsdict.py:100-108 getitem` — dispatches to
        // `getitem_correct` for str keys, else switches strategy.
        // Stub routes through the W_DictObject Vec path until Phase
        // C-3 wires the strategy hierarchy into W_DictObject.
        crate::dictmultiobject::w_dict_lookup(w_dict, w_key)
    }

    unsafe fn setitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef, w_value: PyObjectRef) {
        // `kwargsdict.py:41-67 setitem` + `_setitem_correct_indirection`.
        crate::dictmultiobject::w_dict_store(w_dict, w_key, w_value);
    }

    unsafe fn delitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> bool {
        // `kwargsdict.py:80-83 delitem` — switches to object strategy
        // first (XXX comment: "could do better but is it worth it?").
        crate::dictmultiobject::w_dict_delitem(w_dict, w_key)
    }

    unsafe fn length(&self, w_dict: PyObjectRef) -> usize {
        // `kwargsdict.py:85-86 length`.
        crate::dictmultiobject::w_dict_len(w_dict)
    }

    unsafe fn w_keys(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        // `kwargsdict.py:110-112 w_keys` — returns a copy of keys_w
        // to keep the slice non-resizable.
        crate::dictmultiobject::w_dict_items(w_dict)
            .into_iter()
            .map(|(k, _)| k)
            .collect()
    }

    unsafe fn values(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        // `kwargsdict.py:114-115 values`.
        crate::dictmultiobject::w_dict_items(w_dict)
            .into_iter()
            .map(|(_, v)| v)
            .collect()
    }

    unsafe fn items(&self, w_dict: PyObjectRef) -> Vec<(PyObjectRef, PyObjectRef)> {
        // `kwargsdict.py:117-121 items` — zips the two parallel
        // arrays per (key, value) pair.
        crate::dictmultiobject::w_dict_items(w_dict)
    }

    unsafe fn popitem(&self, w_dict: PyObjectRef) -> Option<(PyObjectRef, PyObjectRef)> {
        // `kwargsdict.py:123-129 popitem` — pop from both arrays.
        let mut items = self.items(w_dict);
        items.pop()
    }

    unsafe fn clear(&self, w_dict: PyObjectRef) {
        // `kwargsdict.py:131-132 clear` — replace dstorage with a
        // fresh empty `([], [])`.
        crate::dictmultiobject::w_dict_clear(w_dict);
    }

    /// `kwargsdict.py:154-156 view_as_kwargs` — expose the two
    /// parallel arrays for `**kwargs` fast unpacking.  Stub returns
    /// `(None, None)`; consumers route through `items()`.
    fn view_as_kwargs(
        &self,
        _w_dict: PyObjectRef,
    ) -> (Option<Vec<String>>, Option<Vec<PyObjectRef>>) {
        (None, None)
    }
}
