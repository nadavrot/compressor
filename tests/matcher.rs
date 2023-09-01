use compressor::lz::matcher::Matcher;

#[test]
fn test_matcher() {
    let input = [
        1, 2, 3, 4, 0, 0, 0, 0, 1, 2, 3, 4, 0, 0, 0, 0, 9, 8, 7, 6, 5, 4, 3, 2,
        1,
    ];

    let mut matcher = Matcher::<1024, 256>::new(&input);
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
