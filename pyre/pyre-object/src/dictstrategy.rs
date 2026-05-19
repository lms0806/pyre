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

    /// `dictmultiobject.py:536-546 popitem` — remove and return an
    /// arbitrary (key, value).  Default `iteritems().next() + delitem`.
    ///
    /// # Safety
    /// `w_dict` must be a valid PyObjectRef.
    unsafe fn popitem(&self, w_dict: PyObjectRef) -> Option<(PyObjectRef, PyObjectRef)> {
        let mut items = self.items(w_dict);
        let (w_key, w_value) = items.drain(..).next()?;
        self.delitem(w_dict, w_key);
        Some((w_key, w_value))
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

    /// `dictmultiobject.py:568-569 view_as_kwargs` — default returns
    /// `(None, None)`.  `KwargsDictStrategy` overrides to expose the
    /// parallel key/value arrays.
    fn view_as_kwargs(
        &self,
        _w_dict: PyObjectRef,
    ) -> (Option<Vec<String>>, Option<Vec<PyObjectRef>>) {
        (None, None)
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

/// `pypy/objspace/std/dictmultiobject.py:1229 BytesDictStrategy`
/// singleton.
pub static BYTES_DICT_STRATEGY: BytesDictStrategy = BytesDictStrategy;

/// `pypy/objspace/std/dictmultiobject.py:1286 UnicodeDictStrategy`
/// singleton.
pub static UNICODE_DICT_STRATEGY: UnicodeDictStrategy = UnicodeDictStrategy;

/// `pypy/objspace/std/dictmultiobject.py:1339 IntDictStrategy`
/// singleton.
pub static INT_DICT_STRATEGY: IntDictStrategy = IntDictStrategy;

/// `dictmultiobject.py:684-743 EmptyDictStrategy`.
///
/// ```python
/// class EmptyDictStrategy(DictStrategy):
///     erase, unerase = rerased.new_erasing_pair("empty")
///
///     def get_empty_storage(self):
///         return self.erase(None)
///
///     def switch_to_correct_strategy(self, w_dict, w_key):
///         ...
///         self.switch_to_object_strategy(w_dict)
///
///     def getitem(self, w_dict, w_key):
///         self.space.hash(w_key)
///         return None
/// ```
///
/// `setitem` / `setitem_str` etc. all force a strategy switch on
/// the empty dict (`switch_to_correct_strategy` → swap to the
/// concrete strategy by key type, then redispatch).  Pyre's stub
/// `setitem` panics until that path lands — empty dicts in pyre
/// today are handled by W_DictObject's existing Vec storage; this
/// strategy is dead code until W_DictObject is migrated.
pub struct EmptyDictStrategy;

impl DictStrategy for EmptyDictStrategy {
    fn get_empty_storage(&self) -> *mut u8 {
        // `erased(None)` — null is the only inhabitant of "empty
        // storage" before a switch installs a real backing.
        std::ptr::null_mut()
    }

    unsafe fn getitem(&self, _w_dict: PyObjectRef, _w_key: PyObjectRef) -> Option<PyObjectRef> {
        // `dictmultiobject.py:738-743` — hash the key (would raise
        // for unhashable types), then return None.  Pyre's hash
        // raise path is wired via `crate::baseobjspace::hash` when
        // the W_DictObject migration consumes this trait.
        None
    }

    unsafe fn setitem(&self, _w_dict: PyObjectRef, _w_key: PyObjectRef, _w_value: PyObjectRef) {
        // `dictmultiobject.py:744-754` — switch_to_correct_strategy
        // then call setitem on the new strategy.  Stubbed until
        // ObjectDictStrategy / UnicodeDictStrategy are ported.
        unimplemented!(
            "EmptyDictStrategy.setitem requires switch_to_correct_strategy + concrete strategies"
        );
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
/// per `dictmultiobject.py:1066`.  Skeleton implementation pending
/// the `W_DictObject.dstorage` migration (Phase C-3); methods route
/// through pyre's existing W_DictObject Vec storage for now.
pub struct BytesDictStrategy;

impl DictStrategy for BytesDictStrategy {
    fn get_empty_storage(&self) -> *mut u8 {
        // `dictmultiobject.py:1244-1247`: erased `{}` with
        // `mark_dict_non_null` hint to JIT.
        let v: Box<Vec<(Vec<u8>, PyObjectRef)>> = Box::new(Vec::new());
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

    /// `dictmultiobject.py:1268-1269 listview_bytes` — exposes the
    /// backing list of bytes keys for the `newlist_bytes` fast path.
    /// Pyre's Vec storage already keeps PyObjectRef keys; consumers
    /// that want raw `Vec<u8>` should bypass the strategy.  Returning
    /// `None` for now matches PyPy's behavior when the dict is empty
    /// or has moved off bytes strategy.
    fn listview_bytes(&self, _w_dict: PyObjectRef) -> Option<Vec<Vec<u8>>> {
        None
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
/// PyObjectRef.  Skeleton implementation pending Phase C-3.
pub struct UnicodeDictStrategy;

impl DictStrategy for UnicodeDictStrategy {
    fn get_empty_storage(&self) -> *mut u8 {
        // `dictmultiobject.py:1302-1304 create_empty_unicode_key_dict`
        // returns an empty `r_dict(unicode_eq, unicode_hash)`.  Pyre's
        // Vec backing is the erased equivalent.
        let v: Box<Vec<(PyObjectRef, PyObjectRef)>> = Box::new(Vec::new());
        Box::into_raw(v) as *mut u8
    }

    unsafe fn getitem(&self, w_dict: PyObjectRef, w_key: PyObjectRef) -> Option<PyObjectRef> {
        crate::dictmultiobject::w_dict_lookup(w_dict, w_key)
    }

    /// `dictmultiobject.py:1311-1313 setitem_str` override — wraps the
    /// key then dispatches to `setitem`.  Same shape as the default;
    /// kept explicit for line-by-line reverse readers.
    unsafe fn setitem_str(&self, w_dict: PyObjectRef, key: &str, w_value: PyObjectRef) {
        let w_key = crate::w_str_new(key);
        self.setitem(w_dict, w_key, w_value);
    }

    /// `dictmultiobject.py:1315-1318 getitem_str` override.
    unsafe fn getitem_str(&self, w_dict: PyObjectRef, key: &str) -> Option<PyObjectRef> {
        let w_key = crate::w_str_new(key);
        self.getitem(w_dict, w_key)
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

    /// `dictmultiobject.py:1323-1334 view_as_kwargs` override — exposes
    /// parallel (keys, values) arrays for `**kwargs` fast unpacking.
    /// Stub returns `(None, None)` for now; concrete consumers route
    /// through `items()` instead.
    fn view_as_kwargs(
        &self,
        _w_dict: PyObjectRef,
    ) -> (Option<Vec<String>>, Option<Vec<PyObjectRef>>) {
        (None, None)
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
/// `pypy/objspace/std/listobject.py`).  Skeleton implementation
/// pending Phase C-3.
pub struct IntDictStrategy;

impl DictStrategy for IntDictStrategy {
    fn get_empty_storage(&self) -> *mut u8 {
        let v: Box<Vec<(i64, PyObjectRef)>> = Box::new(Vec::new());
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

    /// `dictmultiobject.py:1339+ IntDictStrategy.listview_int` — exposes
    /// the backing `Vec<i64>` of keys.  Returns `None` for now;
    /// consumers route through `items()`.
    fn listview_int(&self, _w_dict: PyObjectRef) -> Option<Vec<i64>> {
        None
    }
}
