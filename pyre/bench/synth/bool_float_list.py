N = 1000000


def main():
    i = 0
    acc = 0.0
    lst = [10, 20]
    flst = [1.5, 2.5]
    while i < N:
        flag = i % 2 == 0
        # bool coerced to float in float arithmetic
        acc = acc + flag
        acc = acc + flag * 1.5
        # bool compared against a float (float-pair compare)
        if flag < 0.5:
            acc = acc - 1.0
        # bool as a list index (getitem), int- and float-storage
        acc = acc + lst[flag]
        acc = acc + flst[flag]
        # bool as a list index (setitem); the stored value stays a real int
        lst[flag] = lst[flag] + 1
        i = i + 1
    print(int(acc), lst[0], lst[1])


main()
