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
    /// Reads the runtime typeptr (object class) at offset 0 of
    /// `raw_box` — the lltype `OBJECTPTR` layout that the default
    /// backend uses.  Backends that enable `gcremovetypeptr` route
    /// through `model.py:266+` and override this method to consult
    /// the GC header instead.
    fn cls_of_box(&self, raw_box: i64) -> i64;
}

/// Default `Cpu` implementing `cls_of_box` against the lltype-typeptr-
/// at-offset-0 layout (model.py:199-201).  Production paths that did
/// not install a custom backend hook fall through to this.
pub struct DefaultCpu;

impl Cpu for DefaultCpu {
    fn cls_of_box(&self, raw_box: i64) -> i64 {
        debug_assert!(raw_box != 0, "cls_of_box: null ref");
        // SAFETY: caller has guaranteed `raw_box` is a non-null Ref-typed
        // payload pointer; the lltype OBJECTPTR layout has the typeptr at
        // offset 0 (model.py:200 `box.getref_base().typeptr`).
        unsafe { *(raw_box as *const usize) as i64 }
    }
}

/// `Arc<dyn Cpu>` factory for callers that previously installed a bare
/// `fn(i64) -> i64` hook.  Wraps the fn pointer in a struct that
/// implements `Cpu::cls_of_box` so the trait surface can grow without
/// breaking existing `set_cls_of_box(fn)` call sites.
pub fn cpu_from_cls_of_box_fn(f: fn(i64) -> i64) -> Arc<dyn Cpu> {
    struct ClosureCpu(fn(i64) -> i64);
    impl Cpu for ClosureCpu {
        fn cls_of_box(&self, raw_box: i64) -> i64 {
            (self.0)(raw_box)
        }
    }
    Arc::new(ClosureCpu(f))
}

/// `Arc<dyn Cpu>` to the default lltype backend, for production paths
/// + tests that want the model.py:199-201 typeptr-at-offset-0 read.
pub fn default_cpu() -> Arc<dyn Cpu> {
    Arc::new(DefaultCpu)
}
