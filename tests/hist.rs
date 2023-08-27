use compressor::coding::hist::Histogram;
use compressor::coding::hist::{normalize_to_total_sum, num_bits};

#[test]
fn test_normalize() {
    for i in 2..10 {
        let table_size = 1 << i;
        let mut input: Vec<u32> = vec![10, 10, 24];
        println!("{:?}", input);
        normalize_to_total_sum(&mut input, table_size);
        let sum: u32 = input.iter().sum();
        assert_eq!(sum, table_size);
        println!("{:?}", input);
    }
}

#[test]
fn test_num_bits() {
    assert_eq!(num_bits(3), 2);
    assert_eq!(num_bits(7), 3);
    assert_eq!(num_bits(10), 4);
    assert_eq!(num_bits(256), 9);
}

#[test]
fn test_hist() {
    use rand::thread_rng;
    use rand_distr::{Distribution, Normal};

    let mut data: Vec<u8> = Vec::new();

    let mut rng = thread_rng();
    let normal = Normal::new(35.0_f32, 10.0_f32).unwrap();
    for _ in 0..10000 {
        let v = normal.sample(&mut rng);
        data.push(v as u8);
    }

    let hist: Histogram<256> = Histogram::from_data(&data);
    hist.dump();
}

#[test]
fn test_small_generic() {
    let data: Vec<u8> = vec![
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 10
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 10
        2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, // 12
    ];
    let hist: Histogram<3> = Histogram::from_data(&data);
    hist.dump();
}
