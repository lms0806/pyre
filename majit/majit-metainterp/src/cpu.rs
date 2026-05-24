//! Backend CPU abstraction per `rpython/jit/backend/model.py`.
//!
//! RPython's `AbstractCPU` (model.py:39+) hosts the services every
//! `Optimization` sub-class reaches via `self.optimizer.cpu.<method>()`:
//! `cls_of_box(box)` (model.py:199-201), `bh_*` runtime calls
//! (model.py:209+), GC type-info accessors, and so on.  Pyre currently
//! exposes only `cls_of_box` here; future expansion ports the rest of
//! the AbstractCPU surface onto the same trait so the carrier chain
//! `MetaInterp.cpu → UnrollOpt.cpu → Optimizer.cpu → OptContext.cpu`
//! threads a single trait object instead of an N-tuple of `fn` pointers.

use std::sync::Arc;

use crate::r#box::BoxRef;
use majit_ir::{ArrayDescr, FieldDescr, GcRef, Value};

/// `model.py:39 AbstractCPU` (subset) — services hosted on
/// `optimizer.cpu` and reached from any `Optimization` sub-class.
pub trait Cpu: Send + Sync {
    /// `model.py:199-201 cpu.cls_of_box(box)`:
    ///
    /// ```python
    /// def cls_of_box(self, box):
    ///     obj = lltype.cast_opaque_ptr(OBJECTPTR, box.getref_base())
    ///     return ConstInt(ptr2int(obj.typeptr))
    /// ```
    ///
    /// Reads the runtime typeptr (object class) at offset 0 of the
    /// box's Ref payload — the lltype `OBJECTPTR` layout that the
    /// default backend uses.  Returns 0 when the box does not carry a
    /// concrete `Value::Ref` or when the Ref is null.  Backends that
    /// enable `gcremovetypeptr` route through `model.py:266+` and
    /// override this method to consult the GC header instead.
    fn cls_of_box(&self, box_: &BoxRef) -> i64;

    /// `model.py:199-201 cpu.cls_of_box` lowered to the raw `getref_base`
    /// payload — the `lltype.cast_opaque_ptr(OBJECTPTR, base).typeptr`
    /// step.  Callers that already hold a `GcRef` (e.g. `ConstPtrInfo`
    /// which stores the const ref directly) reach the typeptr read
    /// through this primitive instead of synthesizing a temporary
    /// `BoxRef` chain.  The default `cls_of_box` delegates here.
    fn cls_of_gcref(&self, gcref: GcRef) -> i64;

    /// `model.py:209+ cpu.bh_getfield_gc_i / _r / _f`:
    /// `llmodel.py:467-478 read_int_at_mem / read_ref_at_mem / read_float_at_mem`.
    /// Read the field at `struct_ptr + fielddescr.offset()` honoring
    /// `field_size` + `is_field_signed`. The pure-getfield constant
    /// folder (`executor::execute_nonspec_const`) calls these after
    /// `protect_speculative_field` has validated that `struct_ptr` is
    /// non-null and of the expected type.
    fn bh_getfield_gc_i(&self, struct_ptr: usize, fielddescr: &dyn FieldDescr) -> i64;
    fn bh_getfield_gc_r(&self, struct_ptr: usize, fielddescr: &dyn FieldDescr) -> GcRef;
    fn bh_getfield_gc_f(&self, struct_ptr: usize, fielddescr: &dyn FieldDescr) -> f64;

    /// `llmodel.py:555-567 protect_speculative_field`. Line-by-line:
    ///
    /// ```python
    /// def protect_speculative_field(self, gcptr, fielddescr):
    ///     if not gcptr:
    ///         raise SpeculativeError
    ///     if self.supports_guard_gc_type:
    ///         assert isinstance(fielddescr, FieldDescr)
    ///         sizedescr = fielddescr.parent_descr
    ///         if sizedescr.is_object():
    ///             if (not self.check_is_object(gcptr) or
    ///                 not sizedescr.is_valid_class_for(gcptr)):
    ///                 raise SpeculativeError
    ///         else:
    ///             if self.get_actual_typeid(gcptr) != sizedescr.tid:
    ///                 raise SpeculativeError
    /// ```
    ///
    /// `is_valid_class_for(gcptr)` (`descr.py:217-229`) compares the
    /// runtime typeptr's `subclassrange_min` against the descr's
    /// vtable's `[subclassrange_min, subclassrange_max]` inclusive
    /// interval.  Pyre routes both lookups through
    /// `majit_gc::subclass_range`.
    ///
    /// When `supports_guard_gc_type == False` (the boehm-style pyre
    /// default) only the null check fires, matching `llmodel.py:556-557`.
    ///
    /// **Fail-closed under `supports_guard_gc_type == true`:** PyPy
    /// asserts the field descr has a `parent_descr` and reaches into
    /// it unconditionally; if pyre cannot resolve the same metadata
    /// (parent_descr missing, vtable subclassrange unknown, typeid
    /// unresolvable, downcast fails), the speculative check fails
    /// closed (`Err(())`) so the fold caller declines.  Matching
    /// PyPy's "raise SpeculativeError" on any path where the
    /// type-validity verdict cannot be produced.
    fn protect_speculative_field(
        &self,
        gcptr: GcRef,
        fielddescr: &dyn FieldDescr,
    ) -> Result<(), ()> {
        if gcptr.is_null() {
            return Err(());
        }
        if !majit_gc::supports_guard_gc_type() {
            return Ok(());
        }
        let parent = fielddescr.get_parent_descr().ok_or(())?;
        let sizedescr = parent.as_size_descr().ok_or(())?;
        if sizedescr.is_object() {
            if !majit_gc::check_is_object(gcptr) {
                return Err(());
            }
            // descr.py:217-229 is_valid_class_for — subclassrange
            // containment of gcref's typeptr inside sizedescr.vtable's
            // range.
            let (expected_min, expected_max) =
                majit_gc::subclass_range(sizedescr.vtable()).ok_or(())?;
            let actual_vtable = self.cls_of_gcref(gcptr);
            if actual_vtable == 0 {
                return Err(());
            }
            let (actual_min, _) = majit_gc::subclass_range(actual_vtable as usize).ok_or(())?;
            if !(expected_min <= actual_min && actual_min <= expected_max) {
                return Err(());
            }
        } else {
            let actual_tid = majit_gc::get_actual_typeid(gcptr).ok_or(())?;
            if actual_tid != sizedescr.type_id() {
                return Err(());
            }
        }
        Ok(())
    }

    /// `llmodel.py:569-575 protect_speculative_array`. Line-by-line:
    ///
    /// ```python
    /// def protect_speculative_array(self, gcptr, arraydescr):
    ///     if not gcptr:
    ///         raise SpeculativeError
    ///     if self.supports_guard_gc_type:
    ///         assert isinstance(arraydescr, ArrayDescr)
    ///         if self.get_actual_typeid(gcptr) != arraydescr.tid:
    ///             raise SpeculativeError
    /// ```
    ///
    /// **Fail-closed under `supports_guard_gc_type == true`:**
    /// if `get_actual_typeid(gcptr)` returns `None` (no GC type info
    /// for this gcref), the check fails closed — matching PyPy's
    /// behavior where `None != arraydescr.tid` triggers
    /// SpeculativeError.
    fn protect_speculative_array(
        &self,
        gcptr: GcRef,
        arraydescr: &dyn ArrayDescr,
    ) -> Result<(), ()> {
        if gcptr.is_null() {
            return Err(());
        }
        if !majit_gc::supports_guard_gc_type() {
            return Ok(());
        }
        let actual_tid = majit_gc::get_actual_typeid(gcptr).ok_or(())?;
        if actual_tid != arraydescr.type_id() {
            return Err(());
        }
        Ok(())
    }

    /// `llmodel.py:577-578 protect_speculative_string`:
    ///
    /// ```python
    /// def protect_speculative_string(self, gcptr):
    ///     self.protect_speculative_array(gcptr, self.gc_ll_descr.str_descr)
    /// ```
    ///
    /// PyPy delegates to `protect_speculative_array` with the
    /// `gc_ll_descr.str_descr` cached at backend init.  Pyre does not
    /// carry a typed `str_descr` on the default backend, so under
    /// `supports_guard_gc_type == true` this fails closed — backends
    /// that wire a typed str layout (e.g. a future pyre-object STR
    /// shim) must override to route through their own
    /// `protect_speculative_array(gcptr, str_descr)`.  Under
    /// `supports_guard_gc_type == false` the upstream
    /// `protect_speculative_operation` gate already skips the
    /// string/unicode branch (`mod.rs:5430` "we don't unroll in that
    /// case" port), so only the null check is reachable here.
    fn protect_speculative_string(&self, gcptr: GcRef) -> Result<(), ()> {
        if gcptr.is_null() {
            return Err(());
        }
        if majit_gc::supports_guard_gc_type() {
            return Err(());
        }
        Ok(())
    }

    /// `llmodel.py:580-581 protect_speculative_unicode`.  Mirror of
    /// `protect_speculative_string` for unicode storage; same
    /// fail-closed disposition.
    fn protect_speculative_unicode(&self, gcptr: GcRef) -> Result<(), ()> {
        if gcptr.is_null() {
            return Err(());
        }
        if majit_gc::supports_guard_gc_type() {
            return Err(());
        }
        Ok(())
    }

    /// `model.py:209+ cpu.bh_arraylen_gc` /
    /// `llmodel.py:585-588 read_int_at_mem(array, lendescr.offset, WORD, 1)`.
    /// Default impl reads i64 at `arraydescr.len_descr().offset()`.
    /// Returns `None` when no `len_descr` is registered (matches the
    /// `assert isinstance(arraydescr, ArrayDescr)` failure mode upstream
    /// would hit with a misconfigured descr).
    fn bh_arraylen_gc(&self, array: GcRef, arraydescr: &dyn ArrayDescr) -> Option<i64> {
        let lendescr = arraydescr.len_descr()?;
        let addr = array.0 + lendescr.offset();
        // SAFETY: caller has guaranteed `array` is a valid array gcref
        // and `lendescr.offset()` is the registered length field
        // offset; the length is a 64-bit signed integer per
        // `WORD == 8` upstream (`llmodel.py:587`).
        Some(unsafe { *(addr as *const i64) })
    }

    /// `model.py:209+ cpu.bh_strlen` / `llmodel.py:594-595` and the
    /// rest of the str/unicode read family below.
    ///
    /// **Structural disposition** (not a PRE-EXISTING-ADAPTATION):
    /// pyre's tracer does not emit STRLEN / STRGETITEM / UNICODELEN /
    /// UNICODEGETITEM against concrete Python strings.  Python's `str`
    /// runs as `W_StringObject` and the trace accesses its fields
    /// through `GETFIELD_GC_*` on the wrapper struct.  The
    /// STRLEN/STRGETITEM opcodes are reachable only through the
    /// vstring virtual-string optimizer (`optimizeopt/vstring.rs`),
    /// which folds them inside `VString` info before they ever reach
    /// `execute_nonspec_const`.  The emit helpers at
    /// `history.rs:4296-4307` exist for parity but have no production
    /// caller in pyre (`rg history\.strlen pyre/` returns no matches
    /// as of this commit).
    ///
    /// Returning `None` from the defaults is therefore the
    /// structurally complete answer: any concrete-string fold attempt
    /// would require a typed PyPy `STR` layout (basesize + chars
    /// offset) that pyre does not carry.  Should pyre later expose a
    /// typed string descr, a backend override can implement these
    /// without changing the trait surface.
    fn bh_strlen(&self, _string: GcRef) -> Option<i64> {
        None
    }

    /// See `bh_strlen` for the disposition note.
    fn bh_unicodelen(&self, _unicode: GcRef) -> Option<i64> {
        None
    }

    /// `model.py:209+ cpu.bh_strgetitem` /
    /// `llmodel.py:609-612 read_int_at_mem(string, basesize + index, 1, 0)`.
    /// See `bh_strlen` for the disposition note.
    fn bh_strgetitem(&self, _string: GcRef, _index: i64) -> Option<i64> {
        None
    }

    /// `model.py:209+ cpu.bh_unicodegetitem`.  See `bh_strlen` for the
    /// disposition note.
    fn bh_unicodegetitem(&self, _unicode: GcRef, _index: i64) -> Option<i64> {
        None
    }

    /// `model.py:209+ cpu.bh_getarrayitem_gc_i` /
    /// `llmodel.py:591-594 read_int_at_mem(gcref, ofs + index * size, size, sign)`.
    /// Default impl reads the int item at
    /// `array + ad.base_size() + index * ad.item_size()`, dispatching on
    /// `item_size` × `is_item_signed`.  `index` is assumed to be in
    /// bounds — `protect_speculative_operation` validates first.
    fn bh_getarrayitem_gc_i(&self, array: GcRef, index: i64, ad: &dyn ArrayDescr) -> Option<i64> {
        let addr = array.0 + ad.base_size() + (index as usize) * ad.item_size();
        match (ad.item_size(), ad.is_item_signed()) {
            (8, true) => Some(unsafe { *(addr as *const i64) }),
            (8, false) => Some(unsafe { *(addr as *const u64) as i64 }),
            (4, true) => Some(unsafe { *(addr as *const i32) as i64 }),
            (4, false) => Some(unsafe { *(addr as *const u32) as i64 }),
            (2, true) => Some(unsafe { *(addr as *const i16) as i64 }),
            (2, false) => Some(unsafe { *(addr as *const u16) as i64 }),
            (1, true) => Some(unsafe { *(addr as *const i8) as i64 }),
            (1, false) => Some(unsafe { *(addr as *const u8) as i64 }),
            // llmodel.py:478 `else: raise NotImplementedError(...)` —
            // mirror with `None` so the fold caller skips and the op
            // is emitted verbatim.
            _ => None,
        }
    }

    /// `model.py:209+ cpu.bh_getarrayitem_gc_r` /
    /// `llmodel.py:596-598 read_ref_at_mem(gcref, index * WORD + ofs)`.
    fn bh_getarrayitem_gc_r(&self, array: GcRef, index: i64, ad: &dyn ArrayDescr) -> GcRef {
        let addr = array.0 + ad.base_size() + (index as usize) * ad.item_size();
        GcRef(unsafe { *(addr as *const usize) })
    }

    /// `model.py:209+ cpu.bh_getarrayitem_gc_f` /
    /// `llmodel.py:600-604 read_float_at_mem(gcref, index * FLOATSTORAGE + ofs)`.
    fn bh_getarrayitem_gc_f(&self, array: GcRef, index: i64, ad: &dyn ArrayDescr) -> f64 {
        let addr = array.0 + ad.base_size() + (index as usize) * ad.item_size();
        let bits = unsafe { *(addr as *const u64) };
        f64::from_bits(bits)
    }
}

/// Default `Cpu` implementing `cls_of_box` against the lltype-typeptr-
/// at-offset-0 layout (model.py:199-201).  Production paths that did
/// not install a custom backend hook fall through to this.
pub struct DefaultCpu;

impl Cpu for DefaultCpu {
    fn cls_of_box(&self, box_: &BoxRef) -> i64 {
        // resoperation.py:57-68 walker to the terminal Const.
        match box_.get_box_replacement(false).const_value() {
            Some(Value::Ref(gcref)) if !gcref.is_null() => self.cls_of_gcref(gcref),
            _ => 0,
        }
    }

    fn cls_of_gcref(&self, gcref: GcRef) -> i64 {
        if gcref.is_null() {
            return 0;
        }
        // SAFETY: caller has guaranteed `gcref` is a valid OBJECTPTR
        // payload pointer; the lltype OBJECTPTR layout has the typeptr
        // at offset 0 (model.py:200 `box.getref_base().typeptr`).
        unsafe { *(gcref.0 as *const usize) as i64 }
    }

    fn bh_getfield_gc_i(&self, struct_ptr: usize, fd: &dyn FieldDescr) -> i64 {
        // llmodel.py:467-478 read_int_at_mem signed/unsigned width
        // dispatch. RPython's loop falls through to `else: raise
        // NotImplementedError("size = %d" % size)` when no `itemsize`
        // matches; mirror that with a panic. Callers that may receive
        // exotic field sizes (e.g. the trace-time fold path) MUST
        // pre-filter via `fd.field_size()` before invoking this method.
        let addr = struct_ptr + fd.offset();
        match (fd.field_size(), fd.is_field_signed()) {
            (8, true) => unsafe { *(addr as *const i64) },
            (8, false) => unsafe { *(addr as *const u64) as i64 },
            (4, true) => unsafe { *(addr as *const i32) as i64 },
            (4, false) => unsafe { *(addr as *const u32) as i64 },
            (2, true) => unsafe { *(addr as *const i16) as i64 },
            (2, false) => unsafe { *(addr as *const u16) as i64 },
            (1, true) => unsafe { *(addr as *const i8) as i64 },
            (1, false) => unsafe { *(addr as *const u8) as i64 },
            (size, _) => panic!(
                "bh_getfield_gc_i: unsupported field size {} \
                 (llmodel.py:478 NotImplementedError)",
                size
            ),
        }
    }

    fn bh_getfield_gc_r(&self, struct_ptr: usize, fd: &dyn FieldDescr) -> GcRef {
        // llmodel.py read_ref_at_mem — pointer width.
        let addr = struct_ptr + fd.offset();
        GcRef(unsafe { *(addr as *const usize) })
    }

    fn bh_getfield_gc_f(&self, struct_ptr: usize, fd: &dyn FieldDescr) -> f64 {
        // llmodel.py read_float_at_mem — 64-bit IEEE.
        let addr = struct_ptr + fd.offset();
        let bits = unsafe { *(addr as *const u64) };
        f64::from_bits(bits)
    }
}

/// `Arc<dyn Cpu>` factory for callers that previously installed a bare
/// `fn(i64) -> i64` hook.  Wraps the fn pointer in a struct that
/// extracts the raw Ref value from the BoxRef before invoking the
/// closure, so existing `set_cls_of_box(fn)` call sites continue to
/// receive the raw runtime payload.
pub fn cpu_from_cls_of_box_fn(f: fn(i64) -> i64) -> Arc<dyn Cpu> {
    struct ClosureCpu(fn(i64) -> i64);
    impl Cpu for ClosureCpu {
        fn cls_of_box(&self, box_: &BoxRef) -> i64 {
            let raw = match box_.get_box_replacement(false).const_value() {
                Some(Value::Ref(gcref)) => gcref.0 as i64,
                _ => 0,
            };
            (self.0)(raw)
        }
        fn cls_of_gcref(&self, gcref: GcRef) -> i64 {
            (self.0)(gcref.0 as i64)
        }
        fn bh_getfield_gc_i(&self, struct_ptr: usize, fd: &dyn FieldDescr) -> i64 {
            DefaultCpu.bh_getfield_gc_i(struct_ptr, fd)
        }
        fn bh_getfield_gc_r(&self, struct_ptr: usize, fd: &dyn FieldDescr) -> GcRef {
            DefaultCpu.bh_getfield_gc_r(struct_ptr, fd)
        }
        fn bh_getfield_gc_f(&self, struct_ptr: usize, fd: &dyn FieldDescr) -> f64 {
            DefaultCpu.bh_getfield_gc_f(struct_ptr, fd)
        }
    }
    Arc::new(ClosureCpu(f))
}

/// `Arc<dyn Cpu>` to the default lltype backend, for production paths
/// + tests that want the model.py:199-201 typeptr-at-offset-0 read.
pub fn default_cpu() -> Arc<dyn Cpu> {
    Arc::new(DefaultCpu)
}
