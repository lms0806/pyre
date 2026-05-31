# Exercises the JIT global cell live-read path: a hot loop reads a module
# global that is reassigned to a NON-int object (float) between calls.  A
# float reassign goes through write_cell -> ObjectMutableCell, so the
# compiled loop folds the cell pointer (quasi-immutable version) and must
# read cell.w_value LIVE via GetfieldGcR — an in-place reassign of the same
# cell does not bump the version, so a stale const-fold would return the
# previous value.  Correct output is verified against CPython/PyPy.
N = 300000


def run():
    s = 0.0
    for _ in range(N):
        s += G
    return s


G = 3.0
a = run()
G = 7.0
b = run()
G = 11.0
c = run()
G = 2.0
d = run()
print(a, b, c, d)


def main():
    pass
