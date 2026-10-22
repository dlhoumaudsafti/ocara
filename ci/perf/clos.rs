fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut count = 0;
    let mut add: Box<dyn FnMut(i64)> = Box::new(|k| count += k);
    for i in 0..n { add(i % 3); }
    drop(add);
    println!("{}", count);
}
