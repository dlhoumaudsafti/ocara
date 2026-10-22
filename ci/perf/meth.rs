struct Counter { total: i64 }
impl Counter {
    fn add(&mut self, n: i64) { self.total += self.weight(n); }
    fn weight(&self, n: i64) -> i64 { n * 2 }
}
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut c = Counter { total: 0 };
    for i in 0..n { c.add(i); }
    println!("{}", c.total);
}
