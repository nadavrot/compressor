#!/bin/bash
set -e
set -x

if [ -z "$1" ]
  then echo "Usage: compare.sh input.bin output.bin"
fi

time -p lz4 -1 -c $1 > $2.lz4_1
time -p lz4 -3 -c $1 > $2.lz4_3
time -p lz4 -5 -c $1 > $2.lz4_5
time -p lz4 -9 -c $1 > $2.lz4_9
time -p ~/.bin/lzfse -encode -i $1 -o $2.lzfse
time -p gzip -c $1 > $2.gz
time -p gzip -9 -c $1 > $2.gz9
time -p zstd -c $1 > $2.zstd
time -p zstd -9 -c $1 > $2.zstd9
