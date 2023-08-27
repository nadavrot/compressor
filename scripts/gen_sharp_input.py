#!/bin/python3
import random

# This is the Anti-LZ script. It generates data with a sharp histogram that has
# has no repetitions. Entropy encoders should be able to compress this data, so
# this is a good tool for measuring the quality of entropy encoders for various
# formats.

FILE_SIZE = 1<<17

def get_ch():
    return chr(int(random.gauss(80, 6)))

seen = {""}
packet = "...."
printed = 0
while printed < FILE_SIZE:
    ch = get_ch()
    new_packet = packet[1:] + ch
    if new_packet in seen: continue
    packet = new_packet
    seen.add(packet)
    print(ch, end="")
    printed += 1
