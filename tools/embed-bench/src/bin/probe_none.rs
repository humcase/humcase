//! Size probe without the model runtime: compare its size with `probe_ort`.

fn main() {
    let path = std::env::args().nth(1).unwrap_or_default();
    println!("{}", std::fs::metadata(path).map(|m| m.len()).unwrap_or(0));
}
