N = 100000

METH = '\udc81'   # lone surrogate naming a method on the class
PROP = '\udc82'   # lone surrogate naming a property (data descriptor)
ATTR = '\udc83'   # lone surrogate naming a per-instance attribute


def _meth(self):
    return 1


class P:
    pass


setattr(P, METH, _meth)
setattr(P, PROP, property(lambda self: 2))


def main():
    p = P()
    acc = 0
    i = 0
    # Surrogate-named attribute access through the full descriptor protocol
    # in a JIT-compiled hot loop: a non-data descriptor (function bound
    # through the type MRO) and a data descriptor (property __get__).
    while i < N:
        acc = acc + getattr(p, METH)()
        acc = acc + getattr(p, PROP)
        i = i + 1

    # Post-loop tail running in the already-compiled `main` frame: a
    # per-instance surrogate attribute set degrades the mapdict to the
    # object strategy, then get / __dict__ membership / del round-trip.
    setattr(p, ATTR, 5)
    acc = acc + getattr(p, ATTR)
    acc = acc + (1 if ATTR in p.__dict__ else 0)
    delattr(p, ATTR)
    acc = acc + (0 if hasattr(p, ATTR) else 1)
    print(acc)


main()
