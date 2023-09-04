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
    assert_eq!(bv.pop_word(1), 0);
    assert_eq!(bv.pop_word(1), 1);
    assert_eq!(bv.pop_word(1), 1);
    assert_eq!(bv.len(), 0);

    bv.push_word(0xffaa, 16);
    let lower = bv.pop_word(8);
    let upper = bv.pop_word(8);
    assert_eq!(lower, 0xaa);
    assert_eq!(upper, 0xff);
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
