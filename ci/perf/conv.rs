fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut ti: i64 = 0; let mut tf = 0.0;
    for i in 0..n { ti += i.to_string().parse::<i64>().unwrap(); tf += format!("{}.5", i).parse::<f64>().unwrap(); }
    println!("{} {}", ti, tf);
}
