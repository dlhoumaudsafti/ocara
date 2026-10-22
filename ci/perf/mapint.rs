use std::collections::HashMap;
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut m: HashMap<i64, i64> = HashMap::new();
    for i in 0..n { m.insert(i, i * 2); }
    let mut t = 0;
    for i in 0..n { t += m[&i]; }
    println!("{}", t);
}
