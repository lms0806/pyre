N = 1000000


# UNARY_NEGATIVE in a hot loop lowers to the `unary_negative(value)` HLOp →
# `residual_call_r_r(unary_negative_fn, ListR[value])` through
# opcode_ops::unary_negative_value (mirroring UNARY_INVERT / UNARY_NOT).
# Before the HLOp lowering the flow op `neg` reached the assembler with no
# builder mapping and any `-x` in a JIT-compiled loop panicked.
def main():
    acc = 0
    i = 0
    while i < N:
        x = -i
        acc = acc + x
        i = i + 1
    print(acc)


main()
