"""Phase 6 parity test: module-dict invariants.

PyPy `pypy/objspace/std/dictmultiobject.py:45 W_DictMultiObject` keeps
both `W_DictObject` and `W_ModuleDictObject` user-visible as `dict`,
with `isinstance(x, dict)` true for either.  `__builtins__` and any
module's `__dict__` route through `W_ModuleDictObject` backed by
`ModuleDictStrategy` (`celldict.py:28`).

Pinned contract:
  1. `__builtins__` (or `__builtins__.__dict__` when `__builtins__`
     is the module wrapper) is a `dict` instance,
  2. `globals()` is a `dict` instance,
  3. `len()`, `in`, indexing, iteration all work on it,
  4. mutation through `globals()` is visible by name lookup,
  5. iteration order on globals() matches insertion order
     (PyPy module dicts are insertion-ordered).
"""

# (1) `__builtins__` is dict-like (module wrapper or dict itself).
b = __builtins__
if hasattr(b, "__dict__"):
    bd = b.__dict__
else:
    bd = b  # in pyre / CPython exec contexts __builtins__ may be the dict directly.

assert isinstance(bd, dict), f"type(__builtins__): {type(b).__name__}"


# (2) globals() is a dict.
g = globals()
assert isinstance(g, dict), f"type(globals()): {type(g).__name__}"


# (3) Read / contains / len.
assert "bd" in g
assert "missing_nonexistent_key" not in g
assert g["bd"] is bd
assert len(g) >= 1


# (4) Write via globals() reaches name lookup.
g["_new_var_via_globals"] = 12345
assert _new_var_via_globals == 12345
del g["_new_var_via_globals"]
try:
    _ = _new_var_via_globals
except NameError:
    pass
else:
    assert False, "deleted name must raise NameError"


# (5) Iteration order: insertion order is preserved.
g["_aaa"] = 1
g["_bbb"] = 2
g["_ccc"] = 3
order = [k for k in g if k in ("_aaa", "_bbb", "_ccc")]
assert order == ["_aaa", "_bbb", "_ccc"], f"order: {order!r}"
del g["_aaa"]
del g["_bbb"]
del g["_ccc"]


print("OK")
