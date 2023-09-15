#!/bin/python3
import time, subprocess, sys, os

# This is a small program that creates a CSV file that compares different
# compressors and lists the runtime ane binary size. The program runs: lz4,
# gzip, zstd, compressor and lzfse. These programs need to be in the path.

if len(sys.argv) < 2:
    print("Usage: %s FILENAME" % sys.argv[0])
    sys.exit(1)

input_file = sys.argv[1]
tmp_file = "/tmp/1.bin"

def run(cmd, output=tmp_file):
     start = time.time()
     subprocess.run(cmd, shell=True)
     end = time.time()
     size = os.path.getsize(output)
     os.unlink(output)
     return (end - start, size)

def measure(cmd_pattern, csv_pattern, input_file, output_file, levels):
    if levels:
        for level in range(levels[0], levels[1]):
            res = run(cmd_pattern % (input_file, level, tmp_file))
            print(csv_pattern % res)
        return
    res = run(cmd_pattern % (input_file, tmp_file))
    print(csv_pattern % res)


print("time, lz4, gzip, zstd, cli, lzfse, bzip2, cli-stream")
measure("lz4 -c %s -%d  > %s", "%f, %s,,,,,", input_file, tmp_file,(1,10))
measure("gzip -c %s -%d  > %s", "%f,,%s,,,,", input_file, tmp_file,(1,10))
measure("zstd --single-thread -c %s -%d  > %s", "%f,,,%s,,,", input_file, tmp_file,(1,10))
measure("cli %s --level %d -o %s", "%f,,,,%s,,", input_file, tmp_file,(1,10))
measure("lzfse -encode -i %s -o %s", "%f,,,,,%s,", input_file, tmp_file, None)
measure("bzip2 -c %s -%d  > %s", "%f,,,,,,%s", input_file, tmp_file,(1,10))
measure("cli %s --level %d -o %s --mode lz4", "%f,,,,,,,%s", input_file, tmp_file,(1,10))
