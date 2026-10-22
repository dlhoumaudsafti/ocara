fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let path = format!("/tmp/ocara_perf_file_rs_{}.txt", std::process::id()); let mut text = String::new();
    for i in 0..n { text += &format!("ligne {}\n", i); }
    std::fs::write(&path, &text).unwrap();
    let all = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<String> = all.split('\n').map(String::from).collect();
    let tot: usize = lines.iter().map(|l| l.len()).sum();
    std::fs::remove_file(&path).unwrap();
    println!("{} {}", lines.len(), tot);
}
