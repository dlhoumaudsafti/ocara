trait Weight { fn weight(&self, n: i64) -> i64; }
struct Double; struct Triple;
impl Weight for Double { fn weight(&self, n: i64) -> i64 { n * 2 } }
impl Weight for Triple { fn weight(&self, n: i64) -> i64 { n * 3 } }
#[inline(never)]
fn total(w: &dyn Weight, n: i64) -> i64 { let mut t = 0; for i in 0..n { t += w.weight(i); } t }
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let d: Box<dyn Weight> = Box::new(Double); let t: Box<dyn Weight> = Box::new(Triple);
    println!("{}", total(d.as_ref(), n) + total(t.as_ref(), n));
}
