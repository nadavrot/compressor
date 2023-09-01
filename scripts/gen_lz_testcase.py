#!/bin/python3
import random, sys
from lz4 import block as lzb

# This program finds inputs to the LZ4 compressor where the optimal matcher
# does a much better job than a linear matcher.

syms= ['A','B', 'C']

def get_delta(inp):
    L1 = lzb.compress(bytes(inp), compression=5, mode="high_compression")
    L2 = lzb.compress(bytes(inp), compression=12, mode="high_compression")
    return len(L1) - len(L2), L1, L2

best = 0

# Find an input with a large delta.
while True:
    lst = [ord(random.choice(syms)) for _ in range(20)]
    delta, L1, L2 = get_delta(lst)
    if delta > best:
        best = delta
        print(lst, "len = ", delta)
        print("".join(map(chr,lst)))
        print(L1)
        print(L2)
