//! Thread-local space-callable hooks consumed by pyre-object's dict
//! key dispatch.
//!
//! `pypy/objspace/std/dictmultiobject.py:1195+ ObjectDictStrategy`
//! routes key equality + hashing through `space.eq_w` / `space.hash_w`
//! so user-defined `__eq__` / `__hash__` resolve through the standard
//! comparison protocol.  pyre's `pyre-object` crate is below
//! `pyre-interpreter` in the dependency graph and cannot call into
//! `baseobjspace::eq_w` directly; the hook module mirrors the
//! `gc_hook` pattern: pyre-interpreter (via the pyre-jit init point)
//! installs the trampoline at startup, and pyre-object readers fall
//! back to the builtin equality path when no hook is installed
//! (single-crate tests, snapshot builds before init).
//!
//! Layering: this module has no external dependencies.  Wire-up lives
//! in `pyre-jit::eval`.

use crate::PyObjectRef;
use std::cell::Cell;

/// `pypy/interpreter/baseobjspace.py:823-825 W_ObjectSpace.eq_w`
/// signature: returns `True` when `a` and `b` are equal per the
/// standard `__eq__` protocol.  Unlike PyPy's `space.eq_w` (which
/// raises on `__eq__` errors), the trampoline swallows errors and
/// returns `false` — matching pyre's `baseobjspace::eq_w` shape.
pub type EqWHookFn = unsafe fn(a: PyObjectRef, b: PyObjectRef) -> bool;

thread_local! {
    static EQ_W_HOOK: Cell<Option<EqWHookFn>> = const { Cell::new(None) };
}

/// Install the `eq_w` callback for this thread.  Called from
/// `pyre-jit::eval`'s init alongside the other thread-local hooks.
pub fn register_eq_w_hook(hook: EqWHookFn) {
    EQ_W_HOOK.with(|cell| cell.set(Some(hook)));
}

/// Remove the `eq_w` callback on this thread.
pub fn clear_eq_w_hook() {
    EQ_W_HOOK.with(|cell| cell.set(None));
}

/// Invoke the installed `eq_w` hook.  Returns `None` when no hook is
/// installed (pyre-object lib tests, pre-init snapshot tools); the
/// caller falls back to the limited-type builtin equality so existing
/// behavior is preserved.
///
/// # Safety
/// `a` / `b` must be valid PyObjectRefs (null tolerated as per
/// PyPy's `is_w` shortcut at `baseobjspace.py:818-822`).
#[inline]
pub unsafe fn try_eq_w(a: PyObjectRef, b: PyObjectRef) -> Option<bool> {
    EQ_W_HOOK.with(|cell| cell.get().map(|f| unsafe { f(a, b) }))
}
