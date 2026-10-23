struct Pt { x: i64 }
struct Acc { total: i64 }
impl Pt { fn dist(&self, n: i64) -> i64 { (n - self.x).abs() } }
impl Acc { fn add(&mut self, p: &Pt, n: i64) { self.total += p.dist(n) } }
fn main() {
    let n: i64 = std::env::args().nth(1).unwrap().parse().unwrap();
    let p = Pt { x: 0 }; let mut a = Acc { total: 0 };
    for i in 0..n { a.add(&p, i) }
    println!("{}", a.total);
}
