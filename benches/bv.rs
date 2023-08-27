//! A benchmark for the bitvector.

use compressor::bitvector::Bitvector;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn test_insert_1bit() {
    let mut bv = Bitvector::new();
    for i in 0..1_000_000 {
        bv.push_word(i, 1);
    }

    black_box(bv.len());
}

fn test_insert_n_bits() {
    let mut bv = Bitvector::new();
    for i in 0..1_000_000 {
        bv.push_word(i as u64, 1 + i % 18);
    }

    black_box(bv.len());
}

fn test_pop_1bit() {
    let mut bv = Bitvector::new();

    for i in 0..100_000 {
        bv.push_word(i, 64);
    }

    for _ in 0..100_000 {
        for _ in 0..64 {
            let _ = bv.pop_word(1);
        }
    }

    black_box(bv.len());
}

fn test_pop_n_bit() {
    let mut bv = Bitvector::new();

    for i in 0..1_000_000 {
        bv.push_word(i, 64);
    }

    let mut i = 0;
    while bv.len() > 64 {
        i = (i + 1) % 32;
        let _ = bv.pop_word(i + 2);
    }

    black_box(bv.len());
}

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("insert 1 bit", |b| b.iter(test_insert_1bit));
    c.bench_function("insert n bits", |b| b.iter(test_insert_n_bits));
    c.bench_function("pop 1 bit", |b| b.iter(test_pop_1bit));
    c.bench_function("pop n bit", |b| b.iter(test_pop_n_bit));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
