use compressor::bitvector::Bitvector;

#[test]
fn test_ser_des() {
    for num_bits in 1..120 {
        let mut bv = Bitvector::new();
        let _ = bv.pop_word(0);
        // Generate some data.
        for i in 0..num_bits {
            bv.push_word(i ^ num_bits, 7);
        }

        // Serialize, deserialize and compare the results.
        let mut output = Vec::new();
        let wrote = bv.serialize(&mut output);
        let (bv2, read) = Bitvector::deserialize(&output).unwrap();
        assert_eq!(bv, bv2);
        assert_eq!(read, wrote);
    }
}

#[test]
fn test_clear_upper() {
    assert_eq!(Bitvector::clear_upper_bits(0x3, 1), 1);
    assert_eq!(Bitvector::clear_upper_bits(0x3, 2), 3);
    assert_eq!(Bitvector::clear_upper_bits(0x3, 3), 3);
    assert_eq!(Bitvector::clear_upper_bits(0xffff, 8), 255);
}

#[test]
fn test_bitvector_simple() {
    let mut bv = Bitvector::new();

    assert_eq!(bv.len(), 0);
    bv.push_word(0b1101, 4);
    assert_eq!(bv.len(), 4);
    assert_eq!(bv.pop_word(1), 1);
    assert_eq!(bv.pop_word(1), 1);
    assert_eq!(bv.pop_word(1), 0);
    assert_eq!(bv.pop_word(1), 1);
    assert_eq!(bv.len(), 0);

    bv.push_word(0xffaa, 16);
    let lower = bv.pop_word(8);
    let upper = bv.pop_word(8);
    assert_eq!(lower, 0xff);
    assert_eq!(upper, 0xaa);
}

#[test]
fn test_pop() {
    let mut bv = Bitvector::new();
    // Push and pop a few pairs.
    for i in 0..1000 {
        bv.push_word(i % 3, 1);
        let val = i * 713;
        // Push a full word.
        bv.push_word(val, 64);
        let val2 = bv.pop_word(64);
        assert_eq!(val, val2);
    }
}

#[test]
fn test_bitvector_bug0() {
    let mut bv = Bitvector::new();
    bv.push_word(0x0, 61);
    bv.push_word(0xae, 8);
    let val = bv.pop_word(8);
    assert_eq!(0xae, val);
}

#[test]
fn test_bitvector_bug1() {
    let mut bv = Bitvector::new();

    let mut counter = 1;

    for i in 1..56 {
        // Start the check at different offset in the vector.
        bv.push_word(0x1, i);

        // Outer push.
        counter = (counter * 7) & 0xffffffff;
        bv.push_word(counter, 32);

        // Do an inner push and pop to dirty the upper bits.
        bv.push_word(0xaf, 8);
        let val = bv.pop_word(8);
        assert_eq!(0xaf, val);

        // Check the outer value.
        let popped = bv.pop_word(32);
        assert_eq!(counter & 0xffffffff, popped);
    }
}

#[test]
fn test_bitvector_bug2() {
    let mut bv = Bitvector::new();
    bv.push_word(0x13, 64);
    let val = bv.pop_word(64);
    assert_eq!(0x13, val);
}

#[test]
fn test_bitvector_endianness() {
    let mut bv = Bitvector::new();
    bv.push_word(0x12, 8);
    let a = bv.pop_word(8);
    bv.push_word(0x2, 4);
    bv.push_word(0x1, 4);
    let b = bv.pop_word(8);
    assert_eq!(a, b);
}

#[test]
fn test_bitvector_bug4() {
    let mut bv0 = Bitvector::new();
    let mut bv1 = Bitvector::new();

    let elem: u64 = 5657;
    let num_bits = 6;
    let mut state0 = elem;
    for _ in 0..num_bits {
        bv0.push_word(state0, 1);
        state0 >>= 1;
    }
    let mut state1 = elem;
    bv1.push_word(state1, num_bits);
    state1 >>= num_bits;
    assert_eq!(state1, state0);
    assert_eq!(bv0, bv1);
}

#[test]
fn test_bitvector_bug5() {
    let mut bv = Bitvector::new();
    bv.push_word(0, 2);
    bv.push_word(0, 64);
    let _ = bv.pop_word(64);
    bv.push_word(1, 1);
    bv.push_word(0xff, 64);
    let _ = bv.pop_word(64);
    bv.push_word(2, 1);
    let val = 2 * 713;
    bv.push_word(val, 64);
    let val2 = bv.pop_word(64);
    assert_eq!(val, val2);
}
