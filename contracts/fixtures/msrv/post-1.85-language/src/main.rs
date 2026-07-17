fn main() {
    let candidate = Some(85_u8);
    if let Some(version) = candidate && version == 85 {
        println!("post-1.85 let-chain accepted");
    }
}
