use std::rc::Rc;
struct Node { next: Option<Rc<Node>>, v: i64 }
impl Node { fn get(&self) -> i64 { self.v } }
fn n() -> i64 { std::env::args().nth(1).unwrap().parse().unwrap() }
fn main() {
    let n = n(); let mut nodes: Vec<Rc<Node>> = Vec::new(); let mut prev: Option<Rc<Node>> = None;
    for i in 0..n { let x = Rc::new(Node { next: prev.take(), v: i }); nodes.push(x.clone()); prev = Some(x); }
    let mut tot = 0;
    for _ in 0..5 { for x in &nodes { tot += x.get(); } }
    println!("{}", tot);
    // Chaîne défaite pas à pas : une libération récursive de 10⁶ nœuds déborderait la pile.
    drop(nodes);
    let mut cur = prev;
    while let Some(rc) = cur { cur = Rc::try_unwrap(rc).ok().and_then(|mut node| node.next.take()); }
}
