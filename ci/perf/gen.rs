// Générateur : itérateur.
struct Numbers { i: i64, n: i64 }
impl Iterator for Numbers {
    type Item = i64;
    fn next(&mut self) -> Option<i64> { if self.i < self.n { self.i += 1; Some(self.i - 1) } else { None } }
}
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut t = 0;
    for v in (Numbers { i: 0, n }) { t += v; }
    println!("{}", t);
}
