fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut xs: Vec<i64> = (0..n).map(|i| (i * 7919) % 100003).collect();
    xs.sort();
    println!("{} {}", xs[0], xs[xs.len() - 1]);
}
