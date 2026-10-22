fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let words = [String::from("alpha"), String::from("beta"), String::from("gamma"), String::from("delta")]; let mut hits = 0;
    for _ in 0..n { for w in &words { if w == "gamma" { hits += 1; } } }
    println!("{}", hits);
}
