fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut tot = 0; let name = "ocara";
    for i in 0..n { let s = format!("id={} name={} ok", i, name); tot += s.len(); }
    println!("{}", tot);
}
