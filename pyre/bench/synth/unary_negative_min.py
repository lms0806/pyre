N = 1000000


# UNARY_NEGATIVE on INT_MIN: -INT_MIN overflows the machine-int range, so
# descr_neg (intobject.py:628) takes the long branch and returns 2**63 as a
# W_LongObject.  generated_unary_int_value declines the int fast path at the
# concrete INT_MIN operand and traces the residual long-neg, so the compiled
# loop must agree with the long result rather than wrapping back to INT_MIN.
def main():
    m = -9223372036854775807 - 1  # INT_MIN as a machine int
    acc = 0
    i = 0
    while i < N:
        if -m == 9223372036854775808:
            acc = acc + 1
        i = i + 1
    print(acc)


main()
