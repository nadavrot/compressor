use compressor::lz::matcher::{Matcher, OptimalMatcher};

#[test]
fn test_matcher() {
    let input = [
        1, 2, 3, 4, 0, 0, 0, 0, 1, 2, 3, 4, 0, 0, 0, 0, 9, 8, 7, 6, 5, 4, 3, 2,
        1,
    ];

    let mut matcher = Matcher::<1024, 256, 4, 4, 1>::new(&input);
    let (lit0, mat0) = matcher.next().unwrap();
    let (lit1, mat1) = matcher.next().unwrap();
    assert!(matcher.next().is_none());
    assert_eq!(lit0.start, 0);
    assert_eq!(lit0.len(), 8);
    assert_eq!(mat0.start, 0);
    assert_eq!(mat0.len(), 8);
    assert_eq!(lit1.start, 16);
    assert_eq!(lit1.len(), 9);
    assert_eq!(mat1.start, 0);
    assert_eq!(mat1.len(), 0);
}

#[test]
fn test_matcher_crash() {
    // CBCABAAACACBACACCBCBCABCCBBACAACBBBCBCACCACAACABAABCAACAACBBCCCAC
    let input = [
        67, 66, 67, 65, 66, 65, 65, 65, 67, 65, 67, 66, 65, 67, 65, 67, 67, 66,
        67, 66, 67, 65, 66, 67, 67, 66, 66, 65, 67, 65, 65, 67, 66, 66, 66, 67,
        66, 67, 65, 67, 67, 65, 67, 65, 65, 67, 65, 66, 65, 65, 66, 67, 65, 65,
        67, 65, 65, 67, 66, 66, 67, 67, 67, 65, 67,
    ];
    let mat = Matcher::<65536, 65536, 19, 64, 4>::new(&input);
    for _ in mat {}
}

#[test]
fn test_optimal_sequence() {
    let input = [
        67, 67, 65, 66, 67, 66, 66, 66, 66, 67, 67, 67, 66, 66, 66, 67, 67, 67,
        66, 66, 66, 67, 66, 66, 67,
    ];

    // This sequence:
    // CCABCBBBBCCCBBBCCCBBBCBBC

    // Can be matched as:
    // CCABCBBBBCC CBBB CCCBBBCBBC
    // or:
    // CCABCBBBBCCC BBBCCCBBBC BBC

    let mut mat = Matcher::<65536, 65536, 19, 64, 4>::new(&input);
    let g1 = mat.next().unwrap();
    let g2 = mat.next().unwrap();
    // (0..12, 6..16)
    // (22..25, 0..0)
    assert_eq!(g1.1.len(), 10);
    assert_eq!(g2.1.len(), 0);

    let mut mat = OptimalMatcher::<65536, 65536, 19, 64>::new(&input);
    let g1 = mat.next().unwrap();
    let g2 = mat.next().unwrap();
    // (0..12, 6..16)
    // (22..25, 0..0)
    assert_eq!(g1.1.len(), 10);
    assert_eq!(g2.1.len(), 0);
}

#[test]
fn test_optimal_sequence2() {
    let input = [
        67, 66, 67, 65, 66, 65, 65, 65, 67, 65, 67, 66, 65, 67, 65, 67, 67, 66,
        67, 66, 67, 65, 66, 67, 67, 66, 66, 65, 67, 65, 65, 67, 66, 66, 66, 67,
        66, 67, 65, 67, 67, 65, 67, 65, 65, 67, 65, 66, 65, 65, 66, 67, 65, 65,
        67, 65, 65, 67, 66, 66, 67, 67, 67, 65, 67,
    ];

    // CBCABAAACACBACACCBCBCABCCBBACAACBBBCBCACCACAACABAABCAACAACBBCCCAC
    // Expected:
    // CBCABAAACACB <ACAC> CB <CBCAB> CCB <BACA> ACBB <BCBCA> CC <ACAA> <CABAA> BCA <ACAACBB> CCCAC
    let mat = OptimalMatcher::<65536, 65536, 19, 64>::new(&input);

    let vals: Vec<_> = mat.into_iter().collect();
    assert_eq!(vals.len(), 8);
    assert_eq!(vals[0].0.len(), 12);
    assert_eq!(vals[0].1.len(), 4);

    assert_eq!(vals[1].0.len(), 2);
    assert_eq!(vals[1].1.len(), 5);

    assert_eq!(vals[2].0.len(), 3);
    assert_eq!(vals[2].1.len(), 4);

    assert_eq!(vals[3].0.len(), 4);
    assert_eq!(vals[3].1.len(), 5);

    assert_eq!(vals[4].0.len(), 2);
    assert_eq!(vals[4].1.len(), 4);

    assert_eq!(vals[5].0.len(), 0);
    assert_eq!(vals[5].1.len(), 5);

    assert_eq!(vals[6].0.len(), 3);
    assert_eq!(vals[6].1.len(), 7);

    assert_eq!(vals[7].0.len(), 5);
    assert_eq!(vals[7].1.len(), 0);
}
