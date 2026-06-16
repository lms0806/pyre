# Committed-inline-mutation-then-abort parity.
#
# Same shape as inlined_helper_mutation but with the mutating helpers
# ORDERED so the attribute store commits during recording BEFORE the
# deliberate list.append abort discards the trace: bump(c) traces
# through (its STORE_ATTR side effect commits during the inline
# concrete step), then push(acc, i) hits the append abort and the
# interpreter restarts the iteration from the unadvanced frame,
# re-running bump. The counter must match the iteration count exactly:
# a recording attempt that aborts after the committed store must not
# leave a doubled side effect.
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
        bump(c)
        push(acc, i)
        i = i + 1
    print(len(acc))
    print(c.n)
    print(acc[0], acc[N // 2], acc[N - 1])


main()
