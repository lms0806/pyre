# Pendingblocks Worklist Fixpoint — Loop Header Orthodox Migration

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the pyre-specific loop-header machinery (DONE: retired in `370e441cb2`) with RPython's orthodox `mergeblock` + `closeblock_link`.  Remaining gaps: the generalize arm's `pendingblocks.append` + bytecode re-walk (flowcontext.py:463) — the arm's structural transform and local-rename loop (`:444-447`) are ported, only the worklist re-walk is deferred to task #91; `build_loop_header_state` Input ops only cover locals_w (stack/exc Variables latent).

**Architecture:** Loop back-edges now route through `mergeblock` (flowcontext.py:424).  Before the call, the header block's framestate is restored from `LoopFrame.header_state` because pyre's body-walk may overwrite `block.framestate` during lowering (RPython's SpamBlock.framestate is immutable post-creation).  `closeblock_link` computes `getoutputargs` and creates the Link without `ensure_variable_at_block`.  `pendingblocks.append(newblock)` in the generalize arm is NOT IMPLEMENTED — pyre has no worklist fixpoint loop.

**Tech Stack:** Rust, majit-translate crate (model.rs + front/ast.rs)

---

## Background: Why Direct Approach Failed

An attempt to use `create_block_from_framestate` + `set_goto_from_framestate` for loop headers (2026-05-25 session) hit two blockers:

1. **`ensure_variable_at_block` panic**: Carry-through Variables (locals not in `read ∪ rebound`) may be defined on non-dominating paths. `set_goto_from_framestate` calls `ensure_variable_at_block` for every outputarg, which panics when the predecessor chain can't reach the definition.

2. **`eliminate_empty_blocks` arity mismatch**: After bypassing `ensure_variable_at_block` (via direct `Link::new_mixed`), lazy installer growth during body walk increased header inputargs beyond what the entry link was computed for.

Both blockers trace to the same root: `ensure_variable_at_block` is a pyre adaptation that doesn't exist in RPython. RPython Variables are global objects; Links carry them by identity, no predecessor-chain threading needed.

## File Structure

| File | Responsibility | Changes |
|------|---------------|---------|
| `majit/majit-translate/src/model.rs` | `FunctionGraph::closeblock_link` | New method: `getoutputargs` + `Link::new_mixed` without `ensure_variable_at_block` |
| `majit/majit-translate/src/front/ast.rs` | Loop lowering + `mergeblock` | DONE: Expr::While/Loop/ForLoop use `build_loop_header_state` (state.copy()) + `create_block_from_framestate` + `closeblock_link`; all retired functions removed in `370e441cb2` |

---

### Task 1: Add `FunctionGraph::closeblock_link`

RPython `flowcontext.py:440 currentblock.closeblock(Link(outputargs, block))` — Link creation without predecessor-chain threading. This is the RPython-orthodox way to close a block to a target using framestate-derived args.

**Files:**
- Modify: `majit/majit-translate/src/model.rs` (add method near `set_goto_from_framestate` ~line 3467)

- [ ] **Step 1: Write the test**

```rust
#[test]
fn closeblock_link_creates_link_without_ensure_variable_at_block() {
    // Two blocks: pred → target. Target is a SpamBlock (created via
    // create_block_from_framestate). closeblock_link should create
    // the link via getoutputargs WITHOUT walking predecessor chains.
    // Specifically: a carry-through Variable defined at a block NOT
    // in pred's predecessor chain must NOT panic.
    let mut graph = FunctionGraph::new("closeblock_link_demo");
    let pred = graph.startblock;
    // Create a Variable defined only at pred (not at any predecessor)
    let var_x = graph.push_op_var(pred, OpKind::Input { name: "x".into(), ty: ValueType::Int }, true).unwrap();
    // Build pred_state and target_state
    let pred_state = FrameState {
        entries: vec![Some(var_x.clone())],
        locals_w: vec![Some(Hlvalue::Variable(var_x.clone()))],
        stack: vec![], last_exception: None, blocklist: vec![], next_offset: 0,
    };
    let phi_var = graph.alloc_value_var();
    graph.ensure_variable_registered_void(&phi_var);
    let target_state = FrameState {
        entries: vec![Some(phi_var.clone())],
        locals_w: vec![Some(Hlvalue::Variable(phi_var.clone()))],
        stack: vec![], last_exception: None, blocklist: vec![], next_offset: 0,
    };
    let target = graph.create_block_from_framestate(&target_state);
    // This must NOT panic (set_goto_from_framestate would panic here
    // if var_x were unreachable from pred via ensure_variable_at_block)
    graph.closeblock_link(pred, target, &pred_state, &target_state);
    assert_eq!(graph.block(pred).exits.len(), 1);
    assert_eq!(graph.block(pred).exits[0].args.len(), 1);
}
```

- [ ] **Step 2: Implement `closeblock_link`**

```rust
/// `flowcontext.py:440 currentblock.closeblock(Link(outputargs, block))`
/// — compute link args via `getoutputargs` and create the Link
/// WITHOUT calling `ensure_variable_at_block`.  RPython's `closeblock`
/// never walks predecessor chains; Variables flow through Links
/// directly by identity.
///
/// Use for loop entry/back-edge/continue links where carry-through
/// Variables may not be threadable via the predecessor chain.
/// Forward merges (if/match) should continue using
/// `set_goto_from_framestate` which includes the backfill step.
pub fn closeblock_link(
    &mut self,
    block: BlockId,
    target_block: BlockId,
    pred_state: &FrameState,
    target_state: &FrameState,
) {
    let outputargs = pred_state.getoutputargs(target_state, self);
    let target_inputarg_count = self.block(target_block).inputargs.len();
    assert_eq!(
        outputargs.len(),
        target_inputarg_count,
        "closeblock_link: outputargs.len() ({}) != target.inputargs.len() ({}) — \
         block {:?} → target {:?} on graph {:?}",
        outputargs.len(), target_inputarg_count, block, target_block, self.name,
    );
    let link = Link::new_mixed(outputargs, target_block, None);
    self.set_control_flow_metadata(block, None, vec![link]);
}
```

- [ ] **Step 3: Run tests, verify pass**

Run: `cargo test --lib -p majit-translate 'closeblock_link'`

- [ ] **Step 4: Commit**

---

### Task 2: Wire `mergeblock` to use `closeblock_link` for SpamBlock-to-SpamBlock links

The existing `mergeblock` helper (ast.rs:2997) calls `set_goto_from_framestate` at 3 sites. Two of these (first-visit and current→new generalization) create links from an arbitrary predecessor to a SpamBlock. Replace these with `closeblock_link`.

The third site (dead-candidate rewire at line 3094-3098) uses `ensure_variable_at_block` on the OLD candidate's outputargs. This should also use `closeblock_link` since the old candidate IS a SpamBlock and its Variables are carried by identity.

**Files:**
- Modify: `majit/majit-translate/src/front/ast.rs` (mergeblock method, ~line 2997-3110)

- [ ] **Step 1: Replace all 3 `set_goto_from_framestate` / manual `ensure_variable_at_block` calls in `mergeblock` with `graph.closeblock_link`**

Line 3042: `graph.set_goto_from_framestate(currentblock, newblock, &currentstate, &newstate);`
→ `graph.closeblock_link(currentblock, newblock, &currentstate, &newstate);`

Line 3061: `graph.set_goto_from_framestate(currentblock, cand, &currentstate, &cand_fs);`
→ `graph.closeblock_link(currentblock, cand, &currentstate, &cand_fs);`

Lines 3070-3075: `graph.set_goto_from_framestate(currentblock, newblock, &currentstate, &newstate);`
→ `graph.closeblock_link(currentblock, newblock, &currentstate, &newstate);`

Lines 3094-3098 (dead-candidate rewire): remove `ensure_variable_at_block` loop, replace with:
```rust
let link = Link::new_mixed(old_outputargs, newblock, None);
// Arity assert
assert_eq!(link.args.len(), graph.block(newblock).inputargs.len());
graph.recloseblock(cand, vec![link]);
```

- [ ] **Step 2: Run existing mergeblock tests**

Run: `cargo test --lib -p majit-translate 'mergeblock'`
Expected: 3 existing mergeblock tests pass.

- [ ] **Step 3: Run full test suite**

Run: `cargo test --lib -p majit-translate`
Expected: 2786/0/3 (baseline) — mergeblock is only used by if/match migration paths which have the dry-run gate.

- [ ] **Step 4: Commit**

---

### Task 3: Loop header as SpamBlock via `mergeblock` + `closeblock_link`

Replace `allocate_loop_header_phis` + bare block + manual link args with:
1. `build_loop_header_state` builds the widened FrameState (phi Variables for read∪rebound slots)
2. `create_block_from_framestate` creates the header (ALL Variable slots as inputargs)
3. Emit `OpKind::Input` for phi Variables
4. `closeblock_link` for entry link (NOT `set_goto_from_framestate`)
5. `setstate_at_block` rebinds ctx
6. Back-edge and continue use `closeblock_link` with `getstate`-captured states

The `forbidden_growth` set in `can_thread_variable_to_block` must include the loop header to prevent if/match merges inside the body from growing the header's inputargs via `ensure_variable_at_block`.

**Files:**
- Modify: `majit/majit-translate/src/front/ast.rs`
  - Add `build_loop_header_state` function
  - Rewrite Expr::While, Expr::Loop, Expr::ForLoop
  - Rewrite Expr::Continue
  - Retire `allocate_loop_header_phis`, `header_phi_name_list`, `link_arg_vars_from_ctx`
  - Update test `allocate_loop_header_phis_eager_install_and_pre_loop_link_args`

**Key risk:** `eliminate_empty_blocks` arity mismatch if lazy installer grows a block between header and body during body walk. Mitigation: the header's ALL-Variable inputargs + `setstate_at_block` binding all locals means no lazy install fires for header-visible locals. The `forbidden_growth` set prevents header growth from nested merges.

- [ ] **Step 1: Add `build_loop_header_state` function** (same as attempted earlier — builds widened FrameState with phi Variables)

- [ ] **Step 2: Rewrite Expr::While** — `create_block_from_framestate` + `closeblock_link` for entry, `closeblock_link` for back-edge

- [ ] **Step 3: Rewrite Expr::Loop** — same pattern

- [ ] **Step 4: Rewrite Expr::ForLoop** — same pattern

- [ ] **Step 5: Rewrite Expr::Continue** — `closeblock_link` with header's attached framestate

- [ ] **Step 6: Retire `allocate_loop_header_phis`, `header_phi_name_list`, `link_arg_vars_from_ctx`** — remove dead code, update test

- [ ] **Step 7: Run full test suite**

Run: `cargo test --lib -p majit-translate`
Expected: 2786/0/3 or better.

If arity mismatches surface, diagnose in Task 4.

- [ ] **Step 8: Run pyre/check.py**

Run: `python pyre/check.py`
Expected: dynasm 39/39 + cranelift 39/39

- [ ] **Step 9: Commit**

---

### Task 4: Diagnose and fix `eliminate_empty_blocks` arity mismatches (if any)

If Task 3 Step 7 fails with `simplify.py:513 — len(link.args) == len(link.target.inputargs)`, the root cause is an intermediate block whose inputargs grew via `ensure_variable_at_block` from an if/match merge inside the loop body, but whose incoming link args were computed before the growth.

**Investigation approach:**
1. Capture the graph name, source block, and target block from the panic message
2. Dump `target.inputargs` to see which Variable was added after link creation
3. Trace back to the `ensure_variable_at_block` call that grew the target

**Potential fixes:**
- **A**: Extend `forbidden_growth` to include ALL blocks created by `create_block_from_framestate` (not just loop headers) — prevents ANY SpamBlock growth
- **B**: Post-body-walk fixup: re-close the entry link with updated `getoutputargs` after the body walk completes (mirrors RPython's re-walk via pendingblocks)
- **C**: Add a `closeblock_link`-based back-patch to the entry link after body walk

- [ ] **Step 1: Diagnose which blocks have arity mismatch**
- [ ] **Step 2: Implement fix (A, B, or C depending on diagnosis)**
- [ ] **Step 3: Run full test suite + pyre/check.py**
- [ ] **Step 4: Commit**

---

### Task 5: Generalization + body re-walk via pendingblocks (future session)

When the back-edge state doesn't match the header state (body introduced Variables that widen the header), RPython generalizes: kills the old header, creates a new SpamBlock with the wider state, and re-walks the body.

This requires:
1. `pendingblocks: VecDeque<BlockId>` on `GraphBuildContext`
2. A `record_block` equivalent that walks a single block's AST body
3. Dead-block cleanup for the old header and all blocks created during the first body walk

**This is a multi-session epic** and the least-common codepath (most loops have pre-scannable read∪rebound sets). The `loop_body_locals` pre-scan + `build_loop_header_state` pre-widening handles the common case; generalization only fires for patterns like type-changing rebinds that the pre-scan can't predict.

- [ ] **Step 1: Add `pendingblocks` field to `GraphBuildContext`**
- [ ] **Step 2: Add block-level re-walk capability** (isolate loop body walk into a callable unit)
- [ ] **Step 3: Wire `mergeblock` for back-edge to detect generalization**
- [ ] **Step 4: On generalization, kill old blocks + re-walk body**
- [ ] **Step 5: Tests with type-changing loop rebinds**

---

### Task 6: Retire `loop_body_locals` pre-scan (future session)

Once Task 5's generalization + re-walk is working, `loop_body_locals` becomes redundant — the fixpoint iterator handles all cases. Remove:
- `loop_body_locals` function
- `LoopBodyLocals` struct
- `build_loop_header_state`'s `must_merge` parameter (widen ALL Variable slots)

This makes loop header creation identical to RPython's `make_next_block` — clone the pre-loop state, create SpamBlock, link.

---

## Completion Status

All tasks completed in a single session (2026-05-25):

| Task | Status | Commit |
|------|--------|--------|
| T1: `closeblock_link` primitive | DONE | `1b9bc02130` |
| T2: `mergeblock` adopts `closeblock_link` | DONE | `1b9bc02130` (same commit) |
| T3: Loop header as SpamBlock | DONE | `136dcbbcb4` |
| T4: Arity mismatch fix | DONE | `136dcbbcb4` (fixed: save header_state before cond-eval, restore after) |
| T5: Generalization + body re-walk | PARTIAL / DEFERRED to task #91 | Generalize arm now ports the local-rename loop (`flowcontext.py:444-447`). The `pendingblocks.append` + bytecode re-walk (`:463`) stays unported — pyre's single-pass AST walker has no worklist. On the live loop path this arm is unreachable (header pre-widening makes every slot a fresh Variable → back-edge `union` always `matches()`); the four back-edge call sites `assert_eq!` the returned block equals the header so any future regression fails loud rather than emitting an un-re-walked block. |
| T6: `loop_body_locals` retire | DONE | `55a7722125` |

Final test posture: cargo lib 2790/0/3 + pyre dynasm 39/39 + cranelift 39/39.
