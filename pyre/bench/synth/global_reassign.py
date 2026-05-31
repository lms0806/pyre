# Exercises JIT global-cache invalidation: a hot loop reads a module
# global, then the global is reassigned between calls.  The compiled loop
# must observe each new value (quasi-immutable version invalidation), not a
# stale const-folded value.  Correct output is verified against CPython/PyPy.
N = 300000


def run():
    s = 0
    for _ in range(N):
        s += G
    return s


G = 3
a = run()
G = 7
b = run()
G = 11
c = run()
G = 2
d = run()
print(a, b, c, d)


def main():
    pass
