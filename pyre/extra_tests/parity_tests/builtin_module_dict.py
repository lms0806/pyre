"""Phase 6 parity test: builtin module dicts present as `dict`.

PyPy `pypy/objspace/std/dictmultiobject.py:60-69` constructs every
builtin module's `w_dict` as a `W_ModuleDictObject` backed by
`ModuleDictStrategy` (`celldict.py:28`).  Users see `dict` semantics —
the `W_ModuleDictObject` layout is internal.

Pinned contract:
  1. `sys.__dict__` (and any builtin module's __dict__) is reported as
     `dict` by both `type()` and `isinstance()`,
  2. dict ops (`__contains__`, `__getitem__`, `len`, iteration) all
     work on a builtin module's __dict__,
  3. attribute access on the module routes through the same
     str-keyed map,
  4. the `__name__` entry seeded at module construction is observable
     via either the attribute or the dict.
"""

import sys

# (1) type() / isinstance() agree.
md = sys.__dict__
assert type(md) is dict, f"type(sys.__dict__): {type(md)!r}"
assert isinstance(md, dict), f"isinstance(sys.__dict__, dict): {isinstance(md, dict)}"

# (2) Basic dict ops.
assert "__name__" in md
assert md["__name__"] == "sys"
assert len(md) > 0
some_keys = list(md)
assert "__name__" in some_keys


# (3) Attribute access mirrors __dict__.
assert sys.__name__ == md["__name__"]
assert sys.__name__ == "sys"


# (4) Iteration order respects insertion (PyPy insertion-ordered dicts).
# We don't assert the exact order — sys's init order varies across
# implementations — but iterating and indexing must return the same
# items.
collected = []
for k in md:
    collected.append((k, md[k]))
assert len(collected) == len(md)


# Mutation visibility — write a fresh attribute on `sys`, verify the
# dict reports it, then remove via either path.
sys._pyre_phase5_marker = 999
assert md["_pyre_phase5_marker"] == 999
del md["_pyre_phase5_marker"]
try:
    _ = sys._pyre_phase5_marker
except AttributeError:
    pass
else:
    assert False, "deleted via __dict__ must clear the attribute"


# Reading another builtin module — `math` — to confirm the W_Module
# DictObject path is not specific to `sys`.
import math
assert isinstance(math.__dict__, dict)
assert math.__dict__["__name__"] == "math"
assert math.pi == math.__dict__["pi"]


print("OK")
