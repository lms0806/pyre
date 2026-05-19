"""Phase 6 parity test: function.__globals__ identity across calls.

PyPy `pypy/interpreter/function.py:538-548 fget_func_globals` returns
`self.w_func_globals` directly — every function defined inside the
same module shares ONE `__globals__` identity (the module's `w_dict`).
This is structurally guaranteed by `W_DictMultiObject` being the
single canonical store; pyre's pre-Phase-5-cutover model uses
`dict_storage_to_dict` lazy-mirror_target binding to achieve the same
identity.

Pinned contract:
  1. Two functions defined at module level share `__globals__`,
  2. `f.__globals__` is the same as `globals()`,
  3. A function defined inside another function (nested) still has
     module-level `__globals__` (closure cells go elsewhere),
  4. Mutating `globals()` is visible through `f.__globals__` (and
     vice versa).
"""

def _f():
    return 1

def _g():
    return 2


# (1) Two module-level functions share __globals__.
assert _f.__globals__ is _g.__globals__, "module-level funcs must share __globals__"


# (2) function.__globals__ is globals().
assert _f.__globals__ is globals()


# (3) Nested function still uses module-level __globals__.
def _outer():
    def _inner():
        return 0
    return _inner

_nested = _outer()
assert _nested.__globals__ is _f.__globals__, (
    f"nested function must share module __globals__: "
    f"{_nested.__globals__ is _f.__globals__}"
)


# (4) Mutation visibility.
_f.__globals__["_phase5_mutation_marker"] = 7
assert _phase5_mutation_marker == 7
assert globals()["_phase5_mutation_marker"] == 7
del _f.__globals__["_phase5_mutation_marker"]


print("OK")
