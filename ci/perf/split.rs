fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let line = "alpha,beta,gamma,delta,epsilon,zeta"; let mut tot = 0;
    for _ in 0..n { let parts: Vec<String> = line.split(',').map(String::from).collect(); tot += parts.len(); }
    println!("{}", tot);
}
