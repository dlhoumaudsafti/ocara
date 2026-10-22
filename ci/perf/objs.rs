struct Point { x: i64, y: i64 }
impl Point { fn dist(&self, o: &Point) -> i64 { (self.x - o.x) * (self.x - o.x) + (self.y - o.y) * (self.y - o.y) } }
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let pts: Vec<Box<Point>> = (0..n).map(|i| Box::new(Point { x: i, y: i * 2 })).collect(); let mut tot = 0;
    for i in 1..n as usize { tot += pts[i].dist(&pts[i - 1]); }
    println!("{}", tot);
}
