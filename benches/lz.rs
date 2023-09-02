//! Benchmark for the LZ matcher.

fn get_large_array(items: usize) -> Vec<u8> {
    let mut input = Vec::new();
    let mut v = 0;
    for i in 0..items {
        v += 3;
        input.push((i ^ v) as u8);
    }
    input
}

fn match_large_buffer() {
    let input = get_large_array(1_000_000);
    let matcher = select_matcher(4, &input);
    let cnt = matcher.count();
    black_box(cnt);
}

fn match_small_buffer() {
    let input = get_large_array(1_000);
    let matcher = select_matcher(4, &input);
    let cnt = matcher.count();
    black_box(cnt);
}

use compressor::lz::matcher::select_matcher;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("match_small_buffer", |b| b.iter(match_small_buffer));
    c.bench_function("match_large_buffer", |b| b.iter(match_large_buffer));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
