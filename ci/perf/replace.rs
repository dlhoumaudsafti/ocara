fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut tot = 0;
    for _ in 0..n { tot += "hello world hello".replace("hello", "bye").to_uppercase().len(); }
    println!("{}", tot);
}
