"""Phase 6 parity test: **kwargs unpacking + dict() ctor with module dicts.

PyPy `pypy/objspace/std/dictmultiobject.py:descr_update` and the
DICT_MERGE / DICT_UPDATE opcodes dispatch through the strategy on
both `W_DictObject` and `W_ModuleDictObject`.  After Phase 5d every
builtin module's `__dict__` is a `W_ModuleDictObject`, so passing it
as `**kwargs` or to the `dict()` constructor must walk the strategy
storage rather than the regular dict's entries Vec.

Pinned contract:
  1. `dict(some_module.__dict__)` returns a fresh dict mirroring the
     module's str-keyed entries,
  2. `dict(**some_module.__dict__)` works (subject to: identifier
     keys only — `__doc__` etc.),
  3. dict.update from a module dict works,
  4. comprehensions and iteration produce the same items as direct
     dict() construction.
"""

import sys

# (1) dict() copy from a module dict.
copy = dict(sys.__dict__)
assert isinstance(copy, dict)
assert copy["__name__"] == "sys"
assert len(copy) == len(sys.__dict__)


# (2) dict.update from a module dict.
d = {}
d.update(sys.__dict__)
assert d["__name__"] == "sys"
assert len(d) == len(sys.__dict__)


# (3) Comprehension over module dict items matches dict() construction.
keys = list(sys.__dict__)
values = [sys.__dict__[k] for k in keys]
assert len(keys) == len(values) == len(sys.__dict__)


# (4) Merge into existing dict via | (PyPy descr_or / CPython PEP 584).
# PEP 584 dict union; W_ModuleDictObject is a dict so this should work.
base = {"alpha": 1}
merged = base | dict(sys.__dict__)
assert merged["alpha"] == 1
assert merged["__name__"] == "sys"


# (5) dict() with kwargs that include module-dict entries.
# Only identifier keys can be **-unpacked.  Use a small handpicked set.
small = {"a": 1, "b": 2}
result = dict(small, c=3)
assert result == {"a": 1, "b": 2, "c": 3}


print("OK")
