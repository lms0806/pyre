/// Canonical hint kinds understood by the framework.
///
/// RPython mirrors a single `hint(x, **kwds)` operator (`rlib/jit.py:81`,
/// `flowspace/operation.py:521 add_operator('hint', None, dispatch=1)`)
/// whose kwarg dict picks the behaviour.  Pyre's helper layer cannot
/// carry kwarg dicts on a `Call` shape, so each kwarg-key gets its own
/// dispatch-by-name helper (`hint_access_directly`, `hint_promote`, …)
/// and the variant tag below mirrors the RPython kwarg key.
///
/// RPython equivalents (`rlib/jit.py:81-98`):
///   * `hint(x, access_directly=True)`     → [`AccessDirectly`]
///   * `hint(x, fresh_virtualizable=True)` → [`FreshVirtualizable`]
///   * `hint(x, force_virtualizable=True)` → [`ForceVirtualizable`]
///   * `hint(x, promote=True)`             → [`Promote`]
///     (also reached via `rlib/jit.py:101 promote(x) → hint(x,
///     promote=True)`, the user-facing wrapper).
///
/// TODO: rename `VirtualizableHintKind` → `HintKind`.  The enum now
/// covers `Promote` / `PromoteString` / `PromoteUnicode` in addition
/// to the three vable variants, so the type name is misleading.  The
/// rename is mechanical (rg + sed on the dotted name + the
/// `classify_virtualizable_hint_*` callers) but touches the
/// majit-macros `lower_vable.rs` consumer too, so it's left as a
/// follow-up rather than mixed into this slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualizableHintKind {
    AccessDirectly,
    FreshVirtualizable,
    ForceVirtualizable,
    /// `rlib/jit.py:101 promote(x)` / `hint(x, promote=True)`.  Rewrite
    /// emits `[-live-, <kind>_guard_value(x), Identity(x)]` per
    /// `jit_codewriter/jtransform.py:608-614`; the `<kind>` char is
    /// resolved from the arg's value-kind at rewrite time.
    Promote,
    /// `rlib/jit.py:127 promote_string(x)` / `hint(x,
    /// promote_string=True)`.  RPython emits the 3-input
    /// `str_guard_value/rid>r` op (ref arg + helper fnptr const +
    /// calldescr → result) per `jit_codewriter/jtransform.py:615-631`.
    /// Pyre's rewrite arm currently **panics** because the helper
    /// chain (`OpKind::GuardValue` helper/descr extras +
    /// `_register_extra_helper` port + `assembler.rs` `rid>r` argcode
    /// emit) is not yet wired — recognising the hint kind here lets
    /// the panic carry a TODO message rather than silently dropping
    /// the call into the generic `Call` path.
    PromoteString,
    /// `rlib/jit.py:130 promote_unicode(x)` / `hint(x,
    /// promote_unicode=True)`.  Same `str_guard_value` opname as
    /// `PromoteString` (jit.py:647); discrimination lives in the
    /// `OS_UNIEQ_NONNULL` calldescr.  Same fail-loud state as
    /// `PromoteString` until the helper chain lands.
    PromoteUnicode,
}

/// Classify a function-like symbol as a hint kind.
pub fn classify_virtualizable_hint_segments<'a, I>(segments: I) -> Option<VirtualizableHintKind>
where
    I: IntoIterator<Item = &'a str>,
{
    match segments.into_iter().last().unwrap_or_default() {
        "hint_access_directly" => Some(VirtualizableHintKind::AccessDirectly),
        "hint_fresh_virtualizable" => Some(VirtualizableHintKind::FreshVirtualizable),
        "hint_force_virtualizable" => Some(VirtualizableHintKind::ForceVirtualizable),
        "hint_promote" => Some(VirtualizableHintKind::Promote),
        "hint_promote_string" => Some(VirtualizableHintKind::PromoteString),
        "hint_promote_unicode" => Some(VirtualizableHintKind::PromoteUnicode),
        _ => None,
    }
}

pub fn classify_virtualizable_hint_path(path: &str) -> Option<VirtualizableHintKind> {
    classify_virtualizable_hint_segments(path.split("::"))
}
