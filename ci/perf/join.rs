fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let xs = ["a", "b", "c", "d", "e"]; let mut tot = 0;
    for i in 0..n { tot += xs.join("-").len() + i.to_string().len(); }
    println!("{}", tot);
}
