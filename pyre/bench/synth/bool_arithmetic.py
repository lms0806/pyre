N = 1500000


def main():
    i = 0
    acc = 0
    while i < N:
        flag = i % 2 == 0
        other = i % 3 == 0
        # bool operands in int arithmetic (result is int)
        acc = acc + flag
        acc = acc + flag * 2
        acc = acc - other
        # unary on bool (result is int)
        acc = acc + (-flag)
        acc = acc + (~other)
        # bool bitwise bool stays bool (used as a branch condition)
        if flag & other:
            acc = acc + 1
        if flag | other:
            acc = acc + 2
        if flag ^ other:
            acc = acc + 4
        # bool bitwise int yields an int
        acc = acc + (flag & 3)
        # bool comparison
        if flag < other:
            acc = acc + 8
        i = i + 1
    print(acc)


main()
