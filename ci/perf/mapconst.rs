use std::collections::HashMap;
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n();
    let mut m: HashMap<&str, i64> = [("alpha", 1), ("beta", 2), ("gamma", 3), ("delta", 4), ("epsilon", 5), ("zeta", 6), ("eta", 7), ("theta", 8)].into_iter().collect();
    let mut tot = 0;
    for i in 0..n {
        tot += m["alpha"] + m["delta"] + m["theta"];
        m.insert("beta", i);
        *m.get_mut("zeta").unwrap() += 1;
    }
    println!("{}", tot + m["beta"] + m["zeta"]);
}
