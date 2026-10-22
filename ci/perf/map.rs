use std::collections::HashMap;
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut m: HashMap<String, i64> = HashMap::new();
    for i in 0..n { m.insert(format!("k{}", i), i); }
    let mut tot = 0;
    for i in 0..n { tot += m[&format!("k{}", i)]; }
    println!("{}", tot);
}
