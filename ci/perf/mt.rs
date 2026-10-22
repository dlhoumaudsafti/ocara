use std::sync::Arc;
struct Point { x: i64 }
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n();
    std::thread::spawn(|| {}).join().unwrap();
    let pts: Vec<Arc<Point>> = (0..n).map(|i| Arc::new(Point { x: i })).collect();
    let mut tot = 0;
    for p in &pts { tot += p.x; }
    println!("{}", tot);
}
