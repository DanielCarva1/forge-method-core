// false-test fixture (invalid): assert!(true), tautological assert_eq!,
// empty test body.
//
// This file is expected to trigger:
//   - any-no-assert-true        (line ~8: `assert!(true)`)
//   - any-no-tautological-assert (line ~12: `assert_eq!(1, 1)`)

#[test]
fn placeholder_test() {
    assert!(true);
}

#[test]
fn tautology_test() {
    assert_eq!(1, 1);
}

#[test]
fn real_test() {
    let result = 2 + 2;
    assert_eq!(result, 4);
}
