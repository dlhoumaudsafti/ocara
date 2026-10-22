use std::sync::Mutex;
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let m = Mutex::new(()); let mut c = 0;
    for _ in 0..n { let _g = m.lock().unwrap(); c += 1; }
    println!("{}", c);
}
