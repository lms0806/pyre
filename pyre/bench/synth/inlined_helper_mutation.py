# Inlined-callee shared-heap mutation parity.
#
# A tiny helper mutates a caller-owned list/instance inside a hot while-loop,
# so the tracer inlines the call and the mutating opcode (LIST_APPEND /
# STORE_ATTR) lands on a SHARED heap object. The final counts must match the
# iteration count exactly: a doubled side effect over-counts, a dropped one
# under-counts.
N = 100000


def push(a, v):
    a.append(v)


class Counter:
    def __init__(self):
        self.n = 0


def bump(c):
    c.n = c.n + 1


def main():
    acc = []
    c = Counter()
    i = 0
    while i < N:
        push(acc, i)
        bump(c)
        i = i + 1
    print(len(acc))
    print(c.n)
    print(acc[0], acc[N // 2], acc[N - 1])
    print(sum(acc))


main()
