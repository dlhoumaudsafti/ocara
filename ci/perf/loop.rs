fn fib(n: i64) -> i64 { if n < 2 { n } else { fib(n - 1) + fib(n - 2) } }
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut acc = 0.0;
    for i in 0..n { acc += (i % 7) as f64 * 0.5; }
    println!("{} {}", fib(30), acc);
}
