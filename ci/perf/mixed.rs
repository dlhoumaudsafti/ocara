// Valeur dynamique : énumération entier | flottant.
#[derive(Clone, Copy)]
enum Num { Int(i64), Float(f64) }
fn add(a: Num, b: Num) -> Num {
    match (a, b) {
        (Num::Int(x), Num::Int(y)) => Num::Int(x + y),
        (Num::Int(x), Num::Float(y)) => Num::Float(x as f64 + y),
        (Num::Float(x), Num::Int(y)) => Num::Float(x + y as f64),
        (Num::Float(x), Num::Float(y)) => Num::Float(x + y),
    }
}
fn mul2(a: Num) -> Num { match a { Num::Int(x) => Num::Int(x * 2), Num::Float(x) => Num::Float(x * 2.0) } }
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut t = Num::Int(0);
    for i in 0..n { let x = Num::Int(i); t = add(t, mul2(x)); }
    match t { Num::Int(v) => println!("{}", v), Num::Float(v) => println!("{}", v) }
}
