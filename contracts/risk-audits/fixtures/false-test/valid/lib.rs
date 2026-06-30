// false-test fixture (valid): real assertions, no tautologies,
// no ignored tests without reason.
//
// This file passes every rule in false-test.yaml.

#[test]
fn add_two_returns_four() {
    let result = 2 + 2;
    assert_eq!(result, 4);
}

#[test]
fn parse_int_handles_valid_input() {
    let parsed: Result<i32, _> = "42".parse();
    assert!(parsed.is_ok());
    assert_eq!(parsed.unwrap(), 42);
}

#[test]
fn empty_string_is_empty() {
    let s = String::new();
    assert!(s.is_empty());
}
