fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut xs: Vec<i64> = Vec::new();
    for i in 0..n { xs.push(i); }
    let mut tot = 0;
    for _ in 0..10 { for i in 0..n as usize { tot += xs[i]; } }
    println!("{}", tot);
}
