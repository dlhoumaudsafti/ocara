use std::collections::HashMap;
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let m: HashMap<String, i64> = (0..1000).map(|i| (format!("k{}", i), i)).collect(); let mut tot = 0;
    for _ in 0..n { for (_k, v) in &m { tot += v; } }
    println!("{}", tot);
}
