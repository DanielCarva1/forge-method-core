// security-slop fixture (invalid): hardcoded AWS key, hardcoded password,
// empty expect.
//
// This file is expected to trigger:
//   - any-no-hardcoded-aws-key (line ~9: AKIA...)
//   - any-no-hardcoded-password (line ~10: password = "...")
//   - rust-no-empty-expect       (line ~11: .expect(""))

fn main() {
    let aws_key = "AKIAIOSFODNN7EXAMPLE";
    let password = "hunter2";
    let val: Option<i32> = Some(42);
    let _ = val.expect("");
    println!("{aws_key} {password}");
}
