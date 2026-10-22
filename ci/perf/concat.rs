fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut s = String::new();
    for _ in 0..n { s = s + "x"; }
    println!("{}", s.len());
}
