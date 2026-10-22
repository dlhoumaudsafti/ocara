fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut xs: Vec<i64> = (0..1000).collect(); let mut tot: i64 = 0;
    for i in 0..n {
        if xs.contains(&(i % 1500)) { tot += 1; }
        tot += xs.iter().position(|&x| x == i % 1000).unwrap() as i64;
        let from = (i % 900) as usize;
        let mut part = xs[from..from + 50].to_vec(); part.reverse(); tot += part[0];
        xs.push(i); tot += xs.pop().unwrap();
    }
    println!("{}", tot);
}
