//! `pypy/objspace/std/dictmultiobject.py:462-680 DictStrategy` and
//! `:684-... EmptyDictStrategy` — abstract dict strategy base.
//!
//! Skeleton port: defines the trait surface so concrete strategies
//! (`EmptyDictStrategy`, `ObjectDictStrategy`, `UnicodeDictStrategy`,
//! `BytesDictStrategy`, `IntDictStrategy`, `IdentityDictStrategy`,
//! and the existing `ModuleDictStrategy` in [`crate::celldict`]) can
//! be hung off a single trait dispatch — replacing pyre's flat
//! `W_DictObject.entries: *mut Vec<(PyObjectRef,PyObjectRef)>` layout.
//!
//! No `W_DictObject` callers yet — Phase 5 cutover is staged and the
//! `dstrategy + dstorage` migration of `W_DictObject` lands in a
//! subsequent slice.

#![allow(unsafe_op_in_unsafe_fn)]

use crate::pyobject::PyObjectRef;

/// `dictmultiobject.py:462 DictStrategy` — abstract base.
///
/// Concrete strategies implement the required-no-default methods
/// (`get_empty_storage`, `getitem`, `setitem`, `delitem`, `length`,
/// iterator producers).  Default implementations cover the
/// derived APIs PyPy provides as overridable fallbacks
/// (`getitem_str`, `setitem_str`, `setdefault`, `w_keys`, `values`,
/// `items`, `popitem`, `clear`, `listview_*`, `view_as_kwargs`).
///
/// The PyPy `space` argument is omitted: pyre's `pyre-object` crate
/// has no `ObjSpace` shim, so callers that need str-wrapping
/// (`getitem_str`'s default → `getitem(space.newtext(key))`) call
/// `crate::w_str_new` directly.
pub trait DictStrategy {
    /// `dictmultiobject.py:466-467 get_empty_storage` — return a
    /// freshly-allocated erased storage for this strategy.  Required.
    fn get_empty_storage(&self) -> *mut u8;

    /// `dictmultiobject.py:469-470 getitem` — required.
    ///
    /// # Safety
    /// `w_dict` and `w_key` must be valid PyObjectRef.
    unsafe fn getitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> Option<PyObjectRef>;

    /// `dictmultiobject.py:472-473 getitem_str` — default falls
    /// through to `getitem(w_dict, space.newtext(key))`.
    ///
    /// # Safety
    /// `w_dict` must be a valid PyObjectRef.
    unsafe fn getitem_str(&self, w_dict: PyObjectRef, key: &str) -> Option<PyObjectRef> {
        let w_key = crate::w_str_new(key);
        self.getitem(w_dict, w_key)
    }

    /// `dictmultiobject.py:475-476 setitem` — required.
    ///
    /// # Safety
    /// `w_dict`, `w_key`, and `w_value` must be valid PyObjectRef.
    unsafe fn setitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef, w_value: PyObjectRef);

    /// `dictmultiobject.py:478-479 setitem_str` — default falls
    /// through to `setitem(w_dict, space.newtext(key), w_value)`.
    ///
    /// # Safety
    /// `w_dict` and `w_value` must be valid PyObjectRef.
    unsafe fn setitem_str(&self, w_dict: PyObjectRef, key: &str, w_value: PyObjectRef) {
        let w_key = crate::w_str_new(key);
        self.setitem(w_dict, w_key, w_value);
    }

    /// `dictmultiobject.py:481-482 delitem` — required.
    /// Returns `true` if a key was removed.
    ///
    /// # Safety
    /// `w_dict` and `w_key` must be valid PyObjectRef.
    unsafe fn delitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> bool;

    /// `dictmultiobject.py:484-485 length` — required.
    ///
    /// # Safety
    /// `w_dict` must be a valid PyObjectRef.
    unsafe fn length(&self, w_dict: PyObjectRef) -> usize;

    /// `dictmultiobject.py:487-493 setdefault` — slow default
    /// implementation; concrete strategies override.
    ///
    /// # Safety
    /// `w_dict`, `w_key`, and `w_value` must be valid PyObjectRef.
    unsafe fn setdefault(
        &self,
        w_dict: PyObjectRef,
        w_key: PyObjectRef,
        w_value: PyObjectRef,
    ) -> PyObjectRef {
        if let Some(w_result) = self.getitem(w_dict, w_key) {
            return w_result;
        }
        self.setitem(w_dict, w_key, w_value);
        w_value
    }

    /// `dictmultiobject.py:506-514 w_keys` — collect all keys.
    /// Default builds from the strategy's iterator API.  Concrete
    /// strategies that can short-circuit (e.g. cloning an internal
    /// `Vec`) override.
    ///
    /// # Safety
    /// `w_dict` must be a valid PyObjectRef.
    unsafe fn w_keys(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef>;

    /// `dictmultiobject.py:516-524 values` — collect all values.
    ///
    /// # Safety
    /// `w_dict` must be a valid PyObjectRef.
    unsafe fn values(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef>;

    /// `dictmultiobject.py:526-534 items` — collect (key, value) pairs.
    ///
    /// # Safety
    /// `w_dict` must be a valid PyObjectRef.
    unsafe fn items(&self, w_dict: PyObjectRef) -> Vec<(PyObjectRef, PyObjectRef)>;

    /// `dictmultiobject.py:536-546 popitem` — remove and return the
    /// most recently inserted (key, value) pair.  Python 3.7+ `popitem`
    /// is LIFO (`pypy/objspace/std/dictmultiobject.py:1395
    /// descr_popitem`); the default routes through the strategy's
    /// `items()` and pops the tail.  Concrete strategies override for
    /// O(1) backings (e.g. `ModuleDictStrategy` uses
    /// `IndexMap::pop`).
    ///
    /// # Safety
    /// `w_dict` must be a valid PyObjectRef.
    unsafe fn popitem(&self, w_dict: PyObjectRef) -> Option<(PyObjectRef, PyObjectRef)> {
        let mut items = self.items(w_dict);
        let (w_key, w_value) = items.pop()?;
        self.delitem(w_dict, w_key);
        Some((w_key, w_value))
    }

    /// `dictmultiobject.py:556-557 getiterreversed` — iterate
    /// (key, value) pairs in reverse insertion order (used by
    /// `reversed(dict)` per `pypy/objspace/std/dictmultiobject.py:1494
    /// W_DictMultiObject.descr_reversed`).  Default reverses the
    /// strategy's materialised `items()`; ordered backings override
    /// for streaming reverse iteration.
    ///
    /// # Safety
    /// `w_dict` must be a valid PyObjectRef.
    unsafe fn getiterreversed(&self, w_dict: PyObjectRef) -> Vec<(PyObjectRef, PyObjectRef)> {
        let mut items = self.items(w_dict);
        items.reverse();
        items
    }

    /// `dictmultiobject.py:599-606 W_DictMultiObject.descr_copy` —
    /// strategy-side hook for `dict.copy()`.  Default allocates a
    /// fresh W_DictObject and copies via `items()`; ordered backings
    /// (e.g. `ModuleDictStrategy`) override to unwrap cell-wrapped
    /// values during the copy per `celldict.py:207-216 copy`.
    ///
    /// # Safety
    /// `w_dict` must be a valid PyObjectRef.
    unsafe fn copy(&self, w_dict: PyObjectRef) -> PyObjectRef {
        let new_dict = crate::dictmultiobject::w_dict_new();
        for (k, v) in self.items(w_dict) {
            crate::dictmultiobject::w_dict_store(new_dict, k, v);
        }
        new_dict
    }

    /// `dictmultiobject.py:548-552 clear` — reset to EmptyDictStrategy.
    /// Concrete strategies override to swap the W_DictObject's
    /// `dstrategy` and `dstorage`.
    ///
    /// # Safety
    /// `w_dict` must be a valid PyObjectRef.
    unsafe fn clear(&self, w_dict: PyObjectRef);

    /// `dictmultiobject.py:559-560 listview_bytes` — default returns
    /// `None`; bytes/str strategies override to expose backing list.
    fn listview_bytes(&self, _w_dict: PyObjectRef) -> Option<Vec<Vec<u8>>> {
        None
    }

    /// `dictmultiobject.py:562-563 listview_ascii` — default returns
    /// `None`.
    fn listview_ascii(&self, _w_dict: PyObjectRef) -> Option<Vec<String>> {
        None
    }

    /// `dictmultiobject.py:565-566 listview_int` — default returns
    /// `None`.
    fn listview_int(&self, _w_dict: PyObjectRef) -> Option<Vec<i64>> {
        None
    }

    /// `dictmultiobject.py:777-778 DictStrategy.view_as_kwargs` —
    /// abstract base returns `([], [])` in PyPy's EmptyDictStrategy
    /// override and the upstream default `(None, None)` for non-
    /// kwarg/non-unicode strategies (`:568-569`).  Concrete strategies
    /// override: KwargsDictStrategy returns the parallel arrays
    /// (`kwargsdict.py:154-156`), UnicodeDictStrategy mints arrays
    /// from its r_dict storage (`dictmultiobject.py:1323-1334`).
    ///
    /// # Safety
    /// `w_dict` must be a valid PyObjectRef.
    unsafe fn view_as_kwargs(
        &self,
        _w_dict: PyObjectRef,
    ) -> (Option<Vec<PyObjectRef>>, Option<Vec<PyObjectRef>>) {
        (None, None)
    }

    /// Strategy-side GC trace dispatch for `W_DictObject.dstorage`.
    ///
    /// PRE-EXISTING-ADAPTATION: RPython's translator generates a
    /// per-`rerased`-pair GC trace function from the
    /// `new_erasing_pair("name")` call (`rpython/rlib/rerased.py:24-72`
    /// `new_erasing_pair`), so each PyPy strategy's storage layout
    /// (`r_dict`, `Dict[str, ...]`, `Dict[int, ...]`, parallel arrays)
    /// is traced through its compile-time-known type.  Pyre's
    /// `W_DictObject.dstorage: *mut u8` is erased at runtime, so the
    /// per-strategy trace must dispatch through this trait method.
    ///
    /// Default walks the uniform `Vec<(PyObjectRef, PyObjectRef)>`
    /// shape — every strategy shares that layout until Slices D3/D4
    /// migrate Int/Bytes/Unicode/Kwargs to native typed storages,
    /// where they override `walk_gc_refs` to walk i64-keyed pairs
    /// (skipping the i64 half), `Vec<u8>`-keyed pairs (likewise),
    /// or parallel `keys`/`values` arrays.
    ///
    /// # Safety
    /// `w_dict` must be a valid PyObjectRef pointing at a W_DictObject
    /// whose strategy is `self`.  `visitor` may mutate the visited
    /// PyObjectRef slot to relocate the referenced object during GC.
    unsafe fn walk_gc_refs(&self, w_dict: PyObjectRef, visitor: &mut dyn FnMut(*mut PyObjectRef)) {
        let entries = crate::dictmultiobject::w_dict_object_storage_mut(w_dict);
        for (key, value) in entries.iter_mut() {
            visitor(key as *mut PyObjectRef);
            visitor(value as *mut PyObjectRef);
        }
    }
}

/// `pypy/objspace/std/dictmultiobject.py:1195+ ObjectDictStrategy`
/// process-wide singleton.  PyPy's `space.fromcache(ObjectDictStrategy)`
/// returns the same instance for every space, and `W_DictObject`'s
/// `dstrategy` slot points at it; pyre's `ObjectDictStrategy` is a
/// zero-sized type so a `&'static` reference suffices.  Use this
/// constant whenever the line-by-line port calls
/// `space.fromcache(ObjectDictStrategy)`.
pub static OBJECT_DICT_STRATEGY: ObjectDictStrategy = ObjectDictStrategy;

/// `pypy/objspace/std/dictmultiobject.py:684 EmptyDictStrategy`
/// singleton — same `space.fromcache` semantics as
/// `OBJECT_DICT_STRATEGY`.
pub static EMPTY_DICT_STRATEGY: EmptyDictStrategy = EmptyDictStrategy;

/// `pypy/objspace/std/kwargsdict.py:13 EmptyKwargsDictStrategy`
/// singleton.  Subclass of `EmptyDictStrategy` that promotes
/// directly to `KwargsDictStrategy` on the first unicode setitem,
/// skipping the regular `UnicodeDictStrategy` step.  Selected by
/// `dictmultiobject.py:78-80 allocate_and_init_instance` when the
/// `kwargs=True` flag is set — pyre's `w_dict_new_kwargs()`
/// allocator wires this singleton in for function-call kwarg dicts.
pub static EMPTY_KWARGS_DICT_STRATEGY: EmptyKwargsDictStrategy = EmptyKwargsDictStrategy;

/// `pypy/objspace/std/dictmultiobject.py:1229 BytesDictStrategy`
/// singleton.
pub static BYTES_DICT_STRATEGY: BytesDictStrategy = BytesDictStrategy;

/// `pypy/objspace/std/dictmultiobject.py:1286 UnicodeDictStrategy`
/// singleton.
pub static UNICODE_DICT_STRATEGY: UnicodeDictStrategy = UnicodeDictStrategy;

/// `pypy/objspace/std/dictmultiobject.py:1339 IntDictStrategy`
/// singleton.
pub static INT_DICT_STRATEGY: IntDictStrategy = IntDictStrategy;

/// `dictmultiobject.py:684-790 EmptyDictStrategy`.
///
/// ```python
/// class EmptyDictStrategy(DictStrategy):
///     erase, unerase = rerased.new_erasing_pair("empty")
///
///     def get_empty_storage(self):
///         return self.erase(None)
///
///     def switch_to_correct_strategy(self, w_dict, w_key):
///         if type(w_key) is self.space.StringObjectCls:
///             self.switch_to_bytes_strategy(w_dict)
///             return
///         if type(w_key) is self.space.UnicodeObjectCls:
///             self.switch_to_unicode_strategy(w_dict)
///             return
///         w_type = self.space.type(w_key)
///         if self.space.is_w(w_type, self.space.w_int):
///             self.switch_to_int_strategy(w_dict)
///         elif w_type.compares_by_identity():
///             self.switch_to_identity_strategy(w_dict)
///         else:
///             self.switch_to_object_strategy(w_dict)
///
///     def getitem(self, w_dict, w_key):
///         self.space.hash(w_key)
///         return None
/// ```
///
/// `switch_to_correct_strategy` discriminates the key type:
/// `is_bytes` → Bytes, `is_str` → Unicode, plain `is_int`
/// (excluding bool) → Int.  IdentityDictStrategy is selected by
/// the `compares_by_identity` MRO walker (Slice D5) routed through
/// `dict_eq_hook::COMPARES_BY_IDENTITY_HOOK`; everything else falls
/// into the Object fallback.
pub struct EmptyDictStrategy;

impl EmptyDictStrategy {
    /// `dictmultiobject.py:692-705 switch_to_correct_strategy`.
    ///
    /// # Safety
    /// `w_dict` and `w_key` must be valid PyObjectRef.
    unsafe fn switch_to_correct_strategy(&self, w_dict: PyObjectRef, w_key: PyObjectRef) {
        // `:693-695 type(w_key) is self.space.StringObjectCls`
        // (Python 2 str / Python 3 bytes).
        if crate::is_bytes(w_key) {
            self.switch_to_bytes_strategy(w_dict);
            return;
        }
        // `:696-698 type(w_key) is self.space.UnicodeObjectCls`
        // (Python 2 unicode / Python 3 str).
        if crate::is_str(w_key) {
            crate::dictmultiobject::w_dict_set_strategy(w_dict, &UNICODE_DICT_STRATEGY);
            return;
        }
        // `:700-701 is_w(w_type, self.space.w_int)` — plain int only;
        // bool inherits from int in Python 3 but PyPy's
        // `is_w(type(b), w_int)` is False because `type(True) is bool`.
        if crate::is_int(w_key) && !crate::is_bool(w_key) {
            self.switch_to_int_strategy(w_dict);
            return;
        }
        // `:702-705 elif w_type.compares_by_identity():
        //     self.switch_to_identity_strategy(w_dict)`.
        // Dispatch through `dict_eq_hook::COMPARES_BY_IDENTITY_HOOK`
        // (pyre-interpreter installs the MRO-walking implementation
        // at startup; pyre-object snapshot/lib tests return `None`
        // and fall through to the Object strategy).
        let w_key_type = (*w_key).w_class as PyObjectRef;
        if !w_key_type.is_null() {
            if let Some(true) = crate::dict_eq_hook::try_compares_by_identity(w_key_type) {
                self.switch_to_identity_strategy(w_dict);
                return;
            }
        }
        crate::dictmultiobject::w_dict_set_strategy(w_dict, &OBJECT_DICT_STRATEGY);
    }

    /// `dictmultiobject.py:719-723 switch_to_int_strategy`:
    /// ```python
    /// def switch_to_int_strategy(self, w_dict):
    ///     strategy = self.space.fromcache(IntDictStrategy)
    ///     storage = strategy.get_empty_storage()
    ///     w_dict.set_strategy(strategy)
    ///     w_dict.dstorage = storage
    /// ```
    ///
    /// Pyre additionally drops the legacy empty
    /// `Vec<(PyObjectRef, PyObjectRef)>` allocated by `w_dict_new`
    /// (`malloc_raw`/`Box::into_raw` pair) before installing the
    /// fresh `IntDictStrategy::get_empty_storage` typed `Vec<(i64,
    /// PyObjectRef)>` — PyPy's GC reclaims the unreachable storage
    /// automatically; pyre needs the explicit `Box::from_raw` drop.
    ///
    /// # Safety
    /// `w_dict` must point at a valid `W_DictObject` whose strategy
    /// is currently `EmptyDictStrategy`.
    unsafe fn switch_to_int_strategy(&self, w_dict: PyObjectRef) {
        let dict = &mut *(w_dict as *mut crate::dictmultiobject::W_DictObject);
        if !dict.dstorage.is_null() {
            drop(Box::from_raw(
                dict.dstorage as *mut Vec<(PyObjectRef, PyObjectRef)>,
            ));
        }
        dict.dstorage = INT_DICT_STRATEGY.get_empty_storage();
        dict.dstrategy = &INT_DICT_STRATEGY;
    }

    /// `dictmultiobject.py:707-711 switch_to_bytes_strategy`:
    /// ```python
    /// def switch_to_bytes_strategy(self, w_dict):
    ///     strategy = self.space.fromcache(BytesDictStrategy)
    ///     storage = strategy.get_empty_storage()
    ///     w_dict.set_strategy(strategy)
    ///     w_dict.dstorage = storage
    /// ```
    ///
    /// Pyre drops the legacy empty `Vec<(PyObjectRef, PyObjectRef)>`
    /// before installing the typed `Vec<(Vec<u8>, PyObjectRef)>` from
    /// `BytesDictStrategy::get_empty_storage` — same lifetime
    /// contract as `switch_to_int_strategy`.
    ///
    /// # Safety
    /// Same as [`switch_to_int_strategy`].
    unsafe fn switch_to_bytes_strategy(&self, w_dict: PyObjectRef) {
        let dict = &mut *(w_dict as *mut crate::dictmultiobject::W_DictObject);
        if !dict.dstorage.is_null() {
            drop(Box::from_raw(
                dict.dstorage as *mut Vec<(PyObjectRef, PyObjectRef)>,
            ));
        }
        dict.dstorage = BYTES_DICT_STRATEGY.get_empty_storage();
        dict.dstrategy = &BYTES_DICT_STRATEGY;
    }

    /// `dictmultiobject.py:725-730 switch_to_identity_strategy`:
    /// ```python
    /// def switch_to_identity_strategy(self, w_dict):
    ///     from pypy.objspace.std.identitydict import IdentityDictStrategy
    ///     strategy = self.space.fromcache(IdentityDictStrategy)
    ///     storage = strategy.get_empty_storage()
    ///     w_dict.set_strategy(strategy)
    ///     w_dict.dstorage = storage
    /// ```
    ///
    /// IdentityDictStrategy storage shape matches ObjectDictStrategy
    /// (`Vec<(PyObjectRef, PyObjectRef)>`) — distinction is in the
    /// lookup comparison (raw `==` instead of `dict_keys_equal`).  We
    /// still allocate a fresh box per PyPy's set_strategy + dstorage
    /// reset contract.
    ///
    /// # Safety
    /// Same as [`switch_to_int_strategy`].
    unsafe fn switch_to_identity_strategy(&self, w_dict: PyObjectRef) {
        let dict = &mut *(w_dict as *mut crate::dictmultiobject::W_DictObject);
        if !dict.dstorage.is_null() {
            drop(Box::from_raw(
                dict.dstorage as *mut Vec<(PyObjectRef, PyObjectRef)>,
            ));
        }
        dict.dstorage = crate::identitydict::IDENTITY_DICT_STRATEGY.get_empty_storage();
        dict.dstrategy = &crate::identitydict::IDENTITY_DICT_STRATEGY;
    }
}

/// `kwargsdict.py:13-22 EmptyKwargsDictStrategy(EmptyDictStrategy)`.
///
/// ```python
/// class EmptyKwargsDictStrategy(EmptyDictStrategy):
///     def switch_to_unicode_strategy(self, w_dict):
///         strategy = self.space.fromcache(KwargsDictStrategy)
///         storage = strategy.get_empty_storage()
///         w_dict.set_strategy(strategy)
///         w_dict.dstorage = storage
/// ```
///
/// Rust has no inheritance: the override is expressed as a
/// separate struct whose `setitem` / `setitem_str` route the
/// unicode-key branch into `KwargsDictStrategy` instead of
/// `UnicodeDictStrategy`.  Non-unicode key branches and every
/// other DictStrategy hook delegate to `EMPTY_DICT_STRATEGY`
/// because PyPy's subclass inherits those methods unchanged.
pub struct EmptyKwargsDictStrategy;

impl EmptyKwargsDictStrategy {
    /// `kwargsdict.py:14-18 switch_to_unicode_strategy` — the
    /// subclass override; promotes the W_DictObject straight to
    /// `KWARGS_DICT_STRATEGY` instead of `UNICODE_DICT_STRATEGY`.
    ///
    /// # Safety
    /// `w_dict` must be a W_DictObject whose strategy is
    /// `EMPTY_KWARGS_DICT_STRATEGY`.
    unsafe fn switch_to_kwargs_strategy(&self, w_dict: PyObjectRef) {
        let dict = &mut *(w_dict as *mut crate::dictmultiobject::W_DictObject);
        // No legacy Vec to drop — EmptyKwargsDictStrategy keeps a
        // null `dstorage` like its parent until the first switch
        // installs typed storage.
        dict.dstorage = crate::kwargsdict::KWARGS_DICT_STRATEGY.get_empty_storage();
        dict.dstrategy = &crate::kwargsdict::KWARGS_DICT_STRATEGY;
    }

    /// `dictmultiobject.py:692-705 switch_to_correct_strategy`
    /// duplicated with the unicode branch redirected per PyPy's
    /// subclass MRO dispatch (kwargsdict.py:14-18).
    ///
    /// # Safety
    /// `w_dict` and `w_key` must be valid PyObjectRef.
    unsafe fn switch_to_correct_strategy(&self, w_dict: PyObjectRef, w_key: PyObjectRef) {
        if crate::is_bytes(w_key) {
            EMPTY_DICT_STRATEGY.switch_to_bytes_strategy(w_dict);
            return;
        }
        if crate::is_str(w_key) {
            self.switch_to_kwargs_strategy(w_dict);
            return;
        }
        if crate::is_int(w_key) && !crate::is_bool(w_key) {
            EMPTY_DICT_STRATEGY.switch_to_int_strategy(w_dict);
            return;
        }
        let w_key_type = (*w_key).w_class as PyObjectRef;
        if !w_key_type.is_null() {
            if let Some(true) = crate::dict_eq_hook::try_compares_by_identity(w_key_type) {
                EMPTY_DICT_STRATEGY.switch_to_identity_strategy(w_dict);
                return;
            }
        }
        crate::dictmultiobject::w_dict_set_strategy(w_dict, &OBJECT_DICT_STRATEGY);
    }
}

impl DictStrategy for EmptyKwargsDictStrategy {
    /// `kwargsdict.py:13-14` inherits `EmptyDictStrategy.get_empty_storage`.
    fn get_empty_storage(&self) -> *mut u8 {
        EMPTY_DICT_STRATEGY.get_empty_storage()
    }

    unsafe fn getitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> Option<PyObjectRef> {
        EMPTY_DICT_STRATEGY.getitem(w_dict, w_key)
    }

    unsafe fn setitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef, w_value: PyObjectRef) {
        // `dictmultiobject.py:755-757` — promote via the subclass's
        // `switch_to_correct_strategy`, then setitem on the new
        // strategy.  The kwargs override redirects the unicode
        // branch to KwargsDictStrategy.
        self.switch_to_correct_strategy(w_dict, w_key);
        crate::dictmultiobject::w_dict_store(w_dict, w_key, w_value);
    }

    unsafe fn setitem_str(&self, w_dict: PyObjectRef, key: &str, w_value: PyObjectRef) {
        // `dictmultiobject.py:759-761` setitem_str — caller already
        // chose the str-keyed path, so promote directly to
        // KwargsDictStrategy via the subclass override.
        self.switch_to_kwargs_strategy(w_dict);
        crate::dictmultiobject::w_dict_setitem_str(w_dict, key, w_value);
    }

    unsafe fn delitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> bool {
        EMPTY_DICT_STRATEGY.delitem(w_dict, w_key)
    }

    unsafe fn length(&self, w_dict: PyObjectRef) -> usize {
        EMPTY_DICT_STRATEGY.length(w_dict)
    }

    unsafe fn w_keys(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        EMPTY_DICT_STRATEGY.w_keys(w_dict)
    }

    unsafe fn values(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        EMPTY_DICT_STRATEGY.values(w_dict)
    }

    unsafe fn items(&self, w_dict: PyObjectRef) -> Vec<(PyObjectRef, PyObjectRef)> {
        EMPTY_DICT_STRATEGY.items(w_dict)
    }

    unsafe fn clear(&self, w_dict: PyObjectRef) {
        EMPTY_DICT_STRATEGY.clear(w_dict);
    }

    unsafe fn popitem(&self, w_dict: PyObjectRef) -> Option<(PyObjectRef, PyObjectRef)> {
        EMPTY_DICT_STRATEGY.popitem(w_dict)
    }

    unsafe fn view_as_kwargs(
        &self,
        w_dict: PyObjectRef,
    ) -> (Option<Vec<PyObjectRef>>, Option<Vec<PyObjectRef>>) {
        EMPTY_DICT_STRATEGY.view_as_kwargs(w_dict)
    }
}

impl DictStrategy for EmptyDictStrategy {
    fn get_empty_storage(&self) -> *mut u8 {
        // `erased(None)` — null is the only inhabitant of "empty
        // storage" before a switch installs a real backing.  Pyre's
        // W_DictObject keeps an always-non-null `dstorage` Vec for
        // legacy callers; the EmptyDictStrategy treats it as empty
        // until `switch_to_correct_strategy` flips the dict to a
        // concrete strategy and the Vec starts receiving entries.
        std::ptr::null_mut()
    }

    unsafe fn getitem(&self, _w_dict: PyObjectRef, _w_key: PyObjectRef) -> Option<PyObjectRef> {
        // `dictmultiobject.py:738-743` — hash the key (would raise
        // for unhashable types), then return None.  Pyre's hash
        // raise path is wired via `crate::baseobjspace::hash` when
        // the W_DictObject migration consumes this trait.
        None
    }

    unsafe fn setitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef, w_value: PyObjectRef) {
        // `dictmultiobject.py:755-757 setitem`:
        //   self.switch_to_correct_strategy(w_dict, w_key)
        //   w_dict.setitem(w_key, w_value)
        self.switch_to_correct_strategy(w_dict, w_key);
        crate::dictmultiobject::w_dict_store(w_dict, w_key, w_value);
    }

    unsafe fn setitem_str(&self, w_dict: PyObjectRef, key: &str, w_value: PyObjectRef) {
        // `dictmultiobject.py:759-761 setitem_str`:
        //   self.switch_to_unicode_strategy(w_dict)
        //   w_dict.setitem_str(key, w_value)
        // Unicode-strategy promotion is direct since the caller has
        // already chosen the str-keyed path.
        crate::dictmultiobject::w_dict_set_strategy(w_dict, &UNICODE_DICT_STRATEGY);
        crate::dictmultiobject::w_dict_setitem_str(w_dict, key, w_value);
    }

    unsafe fn delitem(&self, _w_dict: PyObjectRef, _w_key: PyObjectRef) -> bool {
        // `dictmultiobject.py:763-766` — KeyError.  Pyre returns
        // false here; the caller raises.
        false
    }

    unsafe fn length(&self, _w_dict: PyObjectRef) -> usize {
        0
    }

    unsafe fn w_keys(&self, _w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        Vec::new()
    }

    unsafe fn values(&self, _w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        Vec::new()
    }

    unsafe fn items(&self, _w_dict: PyObjectRef) -> Vec<(PyObjectRef, PyObjectRef)> {
        Vec::new()
    }

    unsafe fn clear(&self, _w_dict: PyObjectRef) {
        // Already empty; no-op.
    }

    unsafe fn popitem(&self, _w_dict: PyObjectRef) -> Option<(PyObjectRef, PyObjectRef)> {
        // `dictmultiobject.py:774-775` — KeyError.
        None
    }

    /// `dictmultiobject.py:777-778 EmptyDictStrategy.view_as_kwargs`:
    /// ```python
    /// def view_as_kwargs(self, w_dict):
    ///     return ([], [])
    /// ```
    /// Empty kwarg fast path succeeds with zero entries.
    unsafe fn view_as_kwargs(
        &self,
        _w_dict: PyObjectRef,
    ) -> (Option<Vec<PyObjectRef>>, Option<Vec<PyObjectRef>>) {
        (Some(Vec::new()), Some(Vec::new()))
    }
}

/// `dictmultiobject.py:1195-1226 ObjectDictStrategy`.
///
/// ```python
/// class ObjectDictStrategy(AbstractTypedStrategy, DictStrategy):
///     erase, unerase = rerased.new_erasing_pair("object")
///
///     def wrap(self, unwrapped): return unwrapped
///     def unwrap(self, wrapped): return wrapped
///     def is_correct_type(self, w_obj): return True
///
///     def get_empty_storage(self):
///         new_dict = r_dict(self.space.eq_w, self.space.hash_w, force_non_null=True)
///         return self.erase(new_dict)
///
///     def w_keys(self, w_dict):
///         return self.space.newlist(self.unerase(w_dict.dstorage).keys())
///
///     def setitem_str(self, w_dict, s, w_value):
///         self.setitem(w_dict, self.space.newtext(s), w_value)
///
///     def switch_to_object_strategy(self, w_dict):
///         assert 0, "should be unreachable"
/// ```
///
/// The fallback "any-key" strategy: `is_correct_type` always returns
/// `True`, so any incoming key lands here.  Keys compare via
/// `space.eq_w` / `space.hash_w` — pyre's stub uses
/// [`crate::pyobject::dict_keys_equal`] from
/// [`crate::dictmultiobject`] for parity.
///
/// Skeleton implementation pending the `W_DictObject.dstorage`
/// migration; methods route through pyre's existing
/// `Vec<(PyObjectRef, PyObjectRef)>` so the strategy can be
/// switched on without disturbing call sites.
pub struct ObjectDictStrategy;

impl DictStrategy for ObjectDictStrategy {
    fn get_empty_storage(&self) -> *mut u8 {
        // `dictmultiobject.py:1209-1212`: erased empty r_dict.  Pyre's
        // current storage is a `Vec<(PyObjectRef, PyObjectRef)>` heap
        // alloc — same erased shape via `Box::into_raw`.
        let v: Box<Vec<(PyObjectRef, PyObjectRef)>> = Box::new(Vec::new());
        Box::into_raw(v) as *mut u8
    }

    /// `dictmultiobject.py:1213-1215 getitem` — `self.unerase
    /// (w_dict.dstorage).get(w_key)` plus pyre's `dict_storage_proxy`
    /// storage-first contract for str keys.  Body in
    /// `w_dict_lookup_object_strategy` to avoid recursing through
    /// `w_dict_lookup`.
    unsafe fn getitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> Option<PyObjectRef> {
        crate::dictmultiobject::w_dict_lookup_object_strategy(w_dict, w_key)
    }

    /// `dictmultiobject.py:1216-1218 ObjectDictStrategy.getitem_str` —
    /// upstream just delegates to `getitem` after wrapping the key.
    /// Pyre's W_DictObject additionally carries a `dict_storage_proxy`
    /// for back-mirror dicts (PRE-EXISTING-ADAPTATION, retires with
    /// Phase C-1); the storage-first lookup is preserved here so
    /// strategy-dispatch behavior matches the previous direct
    /// `w_dict_getitem_str` path.
    unsafe fn getitem_str(&self, w_dict: PyObjectRef, key: &str) -> Option<PyObjectRef> {
        crate::dictmultiobject::w_dict_getitem_str_proxy_first(w_dict, key)
    }

    /// `dictmultiobject.py:1220-1221 setitem_str` — `ObjectDictStrategy`
    /// overrides to wrap the str then dispatch to `setitem`.  The
    /// default-trait setitem_str does the same path; keep parity
    /// override here so reverse readers can match.
    unsafe fn setitem_str(&self, w_dict: PyObjectRef, key: &str, w_value: PyObjectRef) {
        let w_key = crate::w_str_new(key);
        self.setitem(w_dict, w_key, w_value);
    }

    /// `dictmultiobject.py:1219 setitem` — `self.unerase
    /// (w_dict.dstorage)[w_key] = w_value` plus the pyre-side
    /// `dict_storage_proxy` sync; body in
    /// `w_dict_store_object_strategy` to avoid recursing through
    /// `w_dict_store`.
    unsafe fn setitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef, w_value: PyObjectRef) {
        crate::dictmultiobject::w_dict_store_object_strategy(w_dict, w_key, w_value);
    }

    /// `dictmultiobject.py:1222 delitem` — `del self.unerase
    /// (w_dict.dstorage)[w_key]` plus the pyre-side `dict_storage_proxy`
    /// sync; body in `w_dict_delitem_object_strategy` to avoid
    /// recursing through `w_dict_delitem`.
    unsafe fn delitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> bool {
        crate::dictmultiobject::w_dict_delitem_object_strategy(w_dict, w_key)
    }

    /// `dictmultiobject.py:1226 length` — `len(self.unerase
    /// (w_dict.dstorage))` plus the pyre-side `dict_storage_proxy`
    /// reconciliation; body in `w_dict_length_object_strategy` to
    /// avoid recursing through `w_dict_len`.
    unsafe fn length(&self, w_dict: PyObjectRef) -> usize {
        crate::dictmultiobject::w_dict_length_object_strategy(w_dict)
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

    /// `dictmultiobject.py:1224-1225 items` — `self.unerase
    /// (w_dict.dstorage).iteritems()` materialised.  Pyre's
    /// W_DictObject pairs the dstorage Vec with a
    /// `dict_storage_proxy` back-mirror that owns str-key authority
    /// when attached; `w_dict_items_object_strategy` handles the merge.
    unsafe fn items(&self, w_dict: PyObjectRef) -> Vec<(PyObjectRef, PyObjectRef)> {
        crate::dictmultiobject::w_dict_items_object_strategy(w_dict)
    }

    /// `dictmultiobject.py:1227-1228 clear` — `self.unerase
    /// (w_dict.dstorage).clear()`.  Direct dstorage truncation +
    /// JIT length-cache reset; `w_dict_clear` (the public wrapper)
    /// handles `dict_storage_proxy` flush bookkeeping.
    unsafe fn clear(&self, w_dict: PyObjectRef) {
        crate::dictmultiobject::w_dict_clear_object_strategy(w_dict);
    }
}

/// `dictmultiobject.py:1229-1278 BytesDictStrategy`.
///
/// ```python
/// class BytesDictStrategy(AbstractTypedStrategy, DictStrategy):
///     erase, unerase = rerased.new_erasing_pair("bytes")
///
///     def wrap(self, unwrapped):
///         return self.space.newbytes(unwrapped)
///     def unwrap(self, wrapped):
///         return self.space.bytes_w(wrapped)
///     def is_correct_type(self, w_obj):
///         return space.is_w(space.type(w_obj), space.w_bytes)
///
///     def get_empty_storage(self):
///         res = {}
///         mark_dict_non_null(res)
///         return self.erase(res)
///
///     def _never_equal_to(self, w_lookup_type):
///         return _never_equal_to_string(self.space, w_lookup_type)
///
///     def listview_bytes(self, w_dict):
///         return self.unerase(w_dict.dstorage).keys()
///
///     def w_keys(self, w_dict):
///         return self.space.newlist_bytes(self.listview_bytes(w_dict))
///
///     def wrapkey(space, key):
///         return space.newbytes(key)
/// ```
///
/// Bytes-keyed dict storage — `is_correct_type` returns true only for
/// W_BytesObject keys; mixed keys force `switch_to_object_strategy`
/// per `dictmultiobject.py:1066`.  Native `Vec<(Vec<u8>, PyObjectRef)>`
/// backing (Slice D4) — the unified-shape adaptation has been
/// retired.
pub struct BytesDictStrategy;

impl BytesDictStrategy {
    /// `dictmultiobject.py:1143-1150 AbstractTypedStrategy.switch_to_object_strategy`
    /// instantiation for BytesDictStrategy — `wrap = newbytes` (`:1234`).
    /// Walks the typed `Vec<(Vec<u8>, _)>`, rebuilds
    /// `Vec<(PyObjectRef, _)>` with each `Vec<u8>` wrapped via
    /// `w_bytes_from_bytes`, drops the typed box.
    ///
    /// # Safety
    /// `w_dict` must point at a valid `W_DictObject` on
    /// [`BYTES_DICT_STRATEGY`].
    unsafe fn switch_to_object_strategy(&self, w_dict: PyObjectRef) {
        let dict = &mut *(w_dict as *mut crate::dictmultiobject::W_DictObject);
        let old = Box::from_raw(dict.dstorage as *mut Vec<(Vec<u8>, PyObjectRef)>);
        let mut new_vec: Vec<(PyObjectRef, PyObjectRef)> = Vec::with_capacity(old.len());
        for (k, v) in old.iter() {
            new_vec.push((crate::w_bytes_from_bytes(k.as_slice()), *v));
        }
        dict.dstorage = Box::into_raw(Box::new(new_vec)) as *mut u8;
        dict.dstrategy = &OBJECT_DICT_STRATEGY;
    }
}

impl DictStrategy for BytesDictStrategy {
    /// `dictmultiobject.py:1244-1247 get_empty_storage` — erased `{}`
    /// with `mark_dict_non_null` hint.  Pyre uses a linear-scan
    /// `Vec<(Vec<u8>, PyObjectRef)>` matching ObjectDictStrategy's
    /// shape pattern.
    fn get_empty_storage(&self) -> *mut u8 {
        let v: Box<Vec<(Vec<u8>, PyObjectRef)>> = Box::new(Vec::new());
        Box::into_raw(v) as *mut u8
    }

    /// `dictmultiobject.py:1095-1103 AbstractTypedStrategy.getitem`.
    unsafe fn getitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> Option<PyObjectRef> {
        if crate::is_bytes(w_key) {
            return crate::dictmultiobject::w_dict_lookup_bytes_strategy(w_dict, w_key);
        }
        // `:1099-1100 _never_equal_to(space.type(w_key))` —
        // `_never_equal_to_string` (`:21-31`) for str-keyed strategies.
        if crate::dictmultiobject::_never_equal_to_string(w_key) {
            return None;
        }
        // `:1101-1103` switch + re-dispatch.
        self.switch_to_object_strategy(w_dict);
        crate::dictmultiobject::w_dict_lookup(w_dict, w_key)
    }

    /// `dictmultiobject.py:1061-1067 AbstractTypedStrategy.setitem`.
    unsafe fn setitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef, w_value: PyObjectRef) {
        if crate::is_bytes(w_key) {
            crate::dictmultiobject::w_dict_store_bytes_strategy(w_dict, w_key, w_value);
            return;
        }
        self.switch_to_object_strategy(w_dict);
        crate::dictmultiobject::w_dict_store(w_dict, w_key, w_value);
    }

    /// `dictmultiobject.py:1069-1071 AbstractTypedStrategy.setitem_str`
    /// — BytesDictStrategy promotes to object on str setitem_str
    /// because str ≠ bytes.
    unsafe fn setitem_str(&self, w_dict: PyObjectRef, key: &str, w_value: PyObjectRef) {
        self.switch_to_object_strategy(w_dict);
        crate::dictmultiobject::w_dict_setitem_str(w_dict, key, w_value);
    }

    /// `dictmultiobject.py:1081-1087 AbstractTypedStrategy.delitem`.
    unsafe fn delitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> bool {
        if crate::is_bytes(w_key) {
            return crate::dictmultiobject::w_dict_delitem_bytes_strategy(w_dict, w_key);
        }
        self.switch_to_object_strategy(w_dict);
        crate::dictmultiobject::w_dict_delitem(w_dict, w_key)
    }

    unsafe fn length(&self, w_dict: PyObjectRef) -> usize {
        crate::dictmultiobject::w_dict_length_bytes_strategy(w_dict)
    }

    unsafe fn w_keys(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        crate::dictmultiobject::w_dict_items_bytes_strategy(w_dict)
            .into_iter()
            .map(|(k, _)| k)
            .collect()
    }

    unsafe fn values(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        crate::dictmultiobject::w_dict_items_bytes_strategy(w_dict)
            .into_iter()
            .map(|(_, v)| v)
            .collect()
    }

    unsafe fn items(&self, w_dict: PyObjectRef) -> Vec<(PyObjectRef, PyObjectRef)> {
        crate::dictmultiobject::w_dict_items_bytes_strategy(w_dict)
    }

    unsafe fn clear(&self, w_dict: PyObjectRef) {
        crate::dictmultiobject::w_dict_clear_bytes_strategy(w_dict);
    }

    /// `dictmultiobject.py:1268-1269 listview_bytes` — `self.unerase
    /// (w_dict.dstorage).keys()`.  Returns the native `Vec<Vec<u8>>`
    /// of keys directly from the typed storage.
    fn listview_bytes(&self, w_dict: PyObjectRef) -> Option<Vec<Vec<u8>>> {
        let entries = unsafe { crate::dictmultiobject::w_dict_bytes_storage(w_dict) };
        Some(entries.iter().map(|(k, _)| k.clone()).collect())
    }

    /// PyPy traces `Dict[str (bytes), W_Root]` only over the value
    /// side (`rerased.new_erasing_pair("bytes")` + auto-generated GC
    /// walker); the Vec<u8> key half is plain bytes and carries no
    /// PyObjectRef.
    unsafe fn walk_gc_refs(&self, w_dict: PyObjectRef, visitor: &mut dyn FnMut(*mut PyObjectRef)) {
        let entries = crate::dictmultiobject::w_dict_bytes_storage_mut(w_dict);
        for (_, value) in entries.iter_mut() {
            visitor(value as *mut PyObjectRef);
        }
    }
}

/// `dictmultiobject.py:1286-1336 UnicodeDictStrategy`.
///
/// ```python
/// class UnicodeDictStrategy(AbstractTypedStrategy, DictStrategy):
///     erase, unerase = rerased.new_erasing_pair("unicode")
///
///     def wrap(self, unwrapped):
///         return unwrapped
///     def unwrap(self, wrapped):
///         assert type(wrapped) is self.space.UnicodeObjectCls
///         return wrapped
///     def is_correct_type(self, w_obj):
///         return type(w_obj) is space.UnicodeObjectCls
///
///     def get_empty_storage(self):
///         res = create_empty_unicode_key_dict(self.space)
///         return self.erase(res)
///
///     def setitem_str(self, w_dict, key, w_value):
///         self.setitem(w_dict, self.space.newtext(key), w_value)
///
///     def getitem_str(self, w_dict, key):
///         assert key is not None
///         return self.getitem(w_dict, self.space.newtext(key))
///
///     def wrapkey(space, key):
///         return key
/// ```
///
/// Unicode-keyed dict storage (Py3's str).  Unlike BytesDictStrategy,
/// `wrap`/`unwrap` are identity functions because keys are already
/// PyObjectRef.  Storage stays on the unified
/// `Vec<(PyObjectRef, PyObjectRef)>` shape because PyPy's
/// `r_dict(unicode_eq, unicode_hash)` (`:1280-1304`) stores
/// W_UnicodeObject keys directly — the unified Vec layout is
/// structurally identical, no native typed migration needed for
/// parity.
pub struct UnicodeDictStrategy;

impl DictStrategy for UnicodeDictStrategy {
    fn get_empty_storage(&self) -> *mut u8 {
        // `dictmultiobject.py:1302-1304 create_empty_unicode_key_dict`
        // returns an empty `r_dict(unicode_eq, unicode_hash)`.  Pyre's
        // Vec backing is the erased equivalent.
        let v: Box<Vec<(PyObjectRef, PyObjectRef)>> = Box::new(Vec::new());
        Box::into_raw(v) as *mut u8
    }

    /// `dictmultiobject.py:1095-1103 AbstractTypedStrategy.getitem`.
    unsafe fn getitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> Option<PyObjectRef> {
        if crate::is_str(w_key) {
            return crate::dictmultiobject::w_dict_lookup_object_strategy(w_dict, w_key);
        }
        if crate::dictmultiobject::_never_equal_to_string(w_key) {
            return None;
        }
        crate::dictmultiobject::w_dict_set_strategy(w_dict, &OBJECT_DICT_STRATEGY);
        crate::dictmultiobject::w_dict_lookup(w_dict, w_key)
    }

    /// `dictmultiobject.py:1311-1313 setitem_str` override — wraps the
    /// key then dispatches to `setitem`.  UnicodeDictStrategy keeps
    /// str keys on the fast path; no promotion.
    unsafe fn setitem_str(&self, w_dict: PyObjectRef, key: &str, w_value: PyObjectRef) {
        let w_key = crate::w_str_new(key);
        crate::dictmultiobject::w_dict_store_object_strategy(w_dict, w_key, w_value);
    }

    /// `dictmultiobject.py:1315-1318 getitem_str` override.
    unsafe fn getitem_str(&self, w_dict: PyObjectRef, key: &str) -> Option<PyObjectRef> {
        let w_key = crate::w_str_new(key);
        crate::dictmultiobject::w_dict_lookup_object_strategy(w_dict, w_key)
    }

    /// `dictmultiobject.py:1061-1067 AbstractTypedStrategy.setitem`.
    unsafe fn setitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef, w_value: PyObjectRef) {
        if crate::is_str(w_key) {
            crate::dictmultiobject::w_dict_store_object_strategy(w_dict, w_key, w_value);
            return;
        }
        crate::dictmultiobject::w_dict_set_strategy(w_dict, &OBJECT_DICT_STRATEGY);
        crate::dictmultiobject::w_dict_store(w_dict, w_key, w_value);
    }

    /// `dictmultiobject.py:1081-1087 AbstractTypedStrategy.delitem`.
    unsafe fn delitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> bool {
        if crate::is_str(w_key) {
            return crate::dictmultiobject::w_dict_delitem_object_strategy(w_dict, w_key);
        }
        crate::dictmultiobject::w_dict_set_strategy(w_dict, &OBJECT_DICT_STRATEGY);
        crate::dictmultiobject::w_dict_delitem(w_dict, w_key)
    }

    unsafe fn length(&self, w_dict: PyObjectRef) -> usize {
        crate::dictmultiobject::w_dict_length_object_strategy(w_dict)
    }

    unsafe fn w_keys(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        crate::dictmultiobject::w_dict_items_object_strategy(w_dict)
            .into_iter()
            .map(|(k, _)| k)
            .collect()
    }

    unsafe fn values(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        crate::dictmultiobject::w_dict_items_object_strategy(w_dict)
            .into_iter()
            .map(|(_, v)| v)
            .collect()
    }

    unsafe fn items(&self, w_dict: PyObjectRef) -> Vec<(PyObjectRef, PyObjectRef)> {
        crate::dictmultiobject::w_dict_items_object_strategy(w_dict)
    }

    unsafe fn clear(&self, w_dict: PyObjectRef) {
        crate::dictmultiobject::w_dict_clear_object_strategy(w_dict);
    }

    /// `dictmultiobject.py:1323-1334 view_as_kwargs` override.
    ///
    /// ```python
    /// @jit.look_inside_iff(lambda self, w_dict: w_dict._unrolling_heuristic())
    /// def view_as_kwargs(self, w_dict):
    ///     d = self.unerase(w_dict.dstorage)
    ///     l = len(d)
    ///     keys, values = [None] * l, [None] * l
    ///     i = 0
    ///     for w_key, val in d.iteritems():
    ///         keys[i] = w_key
    ///         values[i] = val
    ///         i += 1
    ///     return keys, values
    /// ```
    ///
    /// All keys on a UnicodeDictStrategy dict are W_StrObjects, so the
    /// parallel arrays go straight to `argument.py:109-119`'s kwargs
    /// fast path without re-checking types.
    unsafe fn view_as_kwargs(
        &self,
        w_dict: PyObjectRef,
    ) -> (Option<Vec<PyObjectRef>>, Option<Vec<PyObjectRef>>) {
        let items = crate::dictmultiobject::w_dict_items_object_strategy(w_dict);
        let mut keys = Vec::with_capacity(items.len());
        let mut values = Vec::with_capacity(items.len());
        for (w_key, w_val) in items {
            keys.push(w_key);
            values.push(w_val);
        }
        (Some(keys), Some(values))
    }
}

/// `dictmultiobject.py:1339-... IntDictStrategy`.
///
/// ```python
/// class IntDictStrategy(AbstractTypedStrategy, DictStrategy):
///     erase, unerase = rerased.new_erasing_pair("int")
///
///     def wrap(self, unwrapped):
///         return self.space.newint(unwrapped)
///     def unwrap(self, wrapped):
///         from pypy.objspace.std.listobject import plain_int_w
///         return plain_int_w(self.space, wrapped)
///     def is_correct_type(self, w_obj):
///         space = self.space
///         return space.type(w_obj).is_plain_int1()
/// ```
///
/// Int-keyed dict storage.  `is_correct_type` accepts plain
/// `W_IntObject` (not bool, which has its own correctness check at
/// the `listobject.is_plain_int1` predicate per
/// `pypy/objspace/std/listobject.py`).  Native
/// `Vec<(i64, PyObjectRef)>` backing (Slice D3) per
/// `dictmultiobject.py:1339-1374`; `wrap = newint`, `unwrap =
/// plain_int_w`.
pub struct IntDictStrategy;

impl IntDictStrategy {
    /// `dictmultiobject.py:1354-1356 IntDictStrategy.is_correct_type`
    /// — `space.type(w_obj).is_plain_int1()`.  Plain int (not bool)
    /// per `listobject.py:is_plain_int1`.
    #[inline]
    fn is_correct_type(w_key: PyObjectRef) -> bool {
        unsafe { crate::is_int(w_key) && !crate::is_bool(w_key) }
    }

    /// `dictmultiobject.py:1358-1364 _never_equal_to` for int — never
    /// equal to None / bytes / unicode lookup types.  Pyre's bytes/str
    /// are distinct types from int so the cheap pre-check is the same
    /// as `_never_equal_to_string` minus the bool wrinkle (bool == int
    /// in equality, but is_correct_type fences bool out).
    #[inline]
    unsafe fn never_equal_to(w_key: PyObjectRef) -> bool {
        // `space.is_w(w_lookup_type, space.w_NoneType)`:
        if w_key == crate::w_none() {
            return true;
        }
        // bytes / unicode never equal int.
        crate::is_bytes(w_key) || crate::is_str(w_key)
    }

    /// `dictmultiobject.py:1143-1150 AbstractTypedStrategy.switch_to_object_strategy`:
    /// ```python
    /// def switch_to_object_strategy(self, w_dict):
    ///     d = self.unerase(w_dict.dstorage)
    ///     strategy = self.space.fromcache(ObjectDictStrategy)
    ///     d_new = strategy.unerase(strategy.get_empty_storage())
    ///     for key, value in d.iteritems():
    ///         d_new[self.wrap(key)] = value
    ///     w_dict.set_strategy(strategy)
    ///     w_dict.dstorage = strategy.erase(d_new)
    /// ```
    ///
    /// Wraps each i64 key via `wrap = newint` (`:1342`), producing a
    /// fresh `Vec<(PyObjectRef, PyObjectRef)>` and dropping the old
    /// typed `Vec<(i64, PyObjectRef)>` box.
    ///
    /// # Safety
    /// `w_dict` must point at a valid `W_DictObject` on
    /// [`INT_DICT_STRATEGY`].
    unsafe fn switch_to_object_strategy(&self, w_dict: PyObjectRef) {
        let dict = &mut *(w_dict as *mut crate::dictmultiobject::W_DictObject);
        let old = Box::from_raw(dict.dstorage as *mut Vec<(i64, PyObjectRef)>);
        let mut new_vec: Vec<(PyObjectRef, PyObjectRef)> = Vec::with_capacity(old.len());
        for &(k, v) in old.iter() {
            new_vec.push((crate::w_int_new(k), v));
        }
        dict.dstorage = Box::into_raw(Box::new(new_vec)) as *mut u8;
        dict.dstrategy = &OBJECT_DICT_STRATEGY;
    }
}

impl DictStrategy for IntDictStrategy {
    /// `dictmultiobject.py:1339-1352 IntDictStrategy.get_empty_storage`
    /// — `erase({})` (typed `Dict[int, W_Root]` in RPython).  Pyre
    /// uses `Vec<(i64, PyObjectRef)>` with linear scan to match
    /// `w_dict_object_storage`'s shape pattern (PyPy `r_dict` for
    /// Object strategy similarly resolves to a hash table at translation
    /// time; both pyre strategies share the linear-scan adaptation
    /// until ObjectDictStrategy gains a hash-bucket port).
    fn get_empty_storage(&self) -> *mut u8 {
        let v: Box<Vec<(i64, PyObjectRef)>> = Box::new(Vec::new());
        Box::into_raw(v) as *mut u8
    }

    /// `dictmultiobject.py:1095-1103 AbstractTypedStrategy.getitem`.
    unsafe fn getitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> Option<PyObjectRef> {
        if Self::is_correct_type(w_key) {
            return crate::dictmultiobject::w_dict_lookup_int_strategy(w_dict, w_key);
        }
        if Self::never_equal_to(w_key) {
            return None;
        }
        self.switch_to_object_strategy(w_dict);
        crate::dictmultiobject::w_dict_lookup(w_dict, w_key)
    }

    /// `dictmultiobject.py:1061-1067 AbstractTypedStrategy.setitem`.
    unsafe fn setitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef, w_value: PyObjectRef) {
        if Self::is_correct_type(w_key) {
            crate::dictmultiobject::w_dict_store_int_strategy(w_dict, w_key, w_value);
            return;
        }
        self.switch_to_object_strategy(w_dict);
        crate::dictmultiobject::w_dict_store(w_dict, w_key, w_value);
    }

    /// `dictmultiobject.py:1069-1071 setitem_str` — int strategy
    /// promotes on str setitem_str.
    unsafe fn setitem_str(&self, w_dict: PyObjectRef, key: &str, w_value: PyObjectRef) {
        self.switch_to_object_strategy(w_dict);
        crate::dictmultiobject::w_dict_setitem_str(w_dict, key, w_value);
    }

    /// `dictmultiobject.py:1081-1087 AbstractTypedStrategy.delitem`.
    unsafe fn delitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> bool {
        if Self::is_correct_type(w_key) {
            return crate::dictmultiobject::w_dict_delitem_int_strategy(w_dict, w_key);
        }
        self.switch_to_object_strategy(w_dict);
        crate::dictmultiobject::w_dict_delitem(w_dict, w_key)
    }

    unsafe fn length(&self, w_dict: PyObjectRef) -> usize {
        crate::dictmultiobject::w_dict_length_int_strategy(w_dict)
    }

    unsafe fn w_keys(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        crate::dictmultiobject::w_dict_items_int_strategy(w_dict)
            .into_iter()
            .map(|(k, _)| k)
            .collect()
    }

    unsafe fn values(&self, w_dict: PyObjectRef) -> Vec<PyObjectRef> {
        crate::dictmultiobject::w_dict_items_int_strategy(w_dict)
            .into_iter()
            .map(|(_, v)| v)
            .collect()
    }

    unsafe fn items(&self, w_dict: PyObjectRef) -> Vec<(PyObjectRef, PyObjectRef)> {
        crate::dictmultiobject::w_dict_items_int_strategy(w_dict)
    }

    unsafe fn clear(&self, w_dict: PyObjectRef) {
        crate::dictmultiobject::w_dict_clear_int_strategy(w_dict);
    }

    /// `dictmultiobject.py:1366-1367 IntDictStrategy.listview_int` —
    /// `self.unerase(w_dict.dstorage).keys()`.  Returns the native
    /// `Vec<i64>` of keys directly from the typed storage.
    fn listview_int(&self, w_dict: PyObjectRef) -> Option<Vec<i64>> {
        let entries = unsafe { crate::dictmultiobject::w_dict_int_storage(w_dict) };
        Some(entries.iter().map(|&(k, _)| k).collect())
    }

    /// PyPy traces `Dict[int, W_Root]` only over the value side at
    /// translation time (`rerased.new_erasing_pair("integer")` +
    /// auto-generated GC walker); pyre's runtime dispatch mirrors
    /// that by skipping the i64 key half.
    unsafe fn walk_gc_refs(&self, w_dict: PyObjectRef, visitor: &mut dyn FnMut(*mut PyObjectRef)) {
        let entries = crate::dictmultiobject::w_dict_int_storage_mut(w_dict);
        for (_, value) in entries.iter_mut() {
            visitor(value as *mut PyObjectRef);
        }
    }
}
