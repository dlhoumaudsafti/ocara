// Exception : panique rattrapée (catch_unwind), message de panique masqué.
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let n = n(); let mut c = 0;
    for _ in 0..n { if std::panic::catch_unwind(|| panic!("x")).is_err() { c += 1; } }
    println!("{}", c);
}
