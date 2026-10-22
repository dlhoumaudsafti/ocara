// Exception de classe : panique portant une valeur typée, rattrapée et testée.
struct AppError { _message: String, _code: i64 }
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let n = n(); let mut c = 0;
    for i in 0..n {
        let r = std::panic::catch_unwind(|| std::panic::panic_any(AppError { _message: "x".into(), _code: i }));
        if let Err(e) = r { if e.is::<AppError>() { c += 1; } }
    }
    println!("{}", c);
}
