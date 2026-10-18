//! Collecte des cycles pendant que d'autres threads Ocara tournent — voir
//! docs/roadmap.d/memoire-refcount.md (« Collecte concurrente »).
//!
//! Le collecteur ne modifie aucun compte : il relève, pour le sous-graphe
//! atteignable depuis les racines candidates, le compte de chaque valeur et
//! ses références internes (arêtes venant du sous-graphe). Est vivante une
//! valeur dont le compte dépasse ses références internes (une référence
//! vient d'ailleurs : pile, global, valeur hors sous-graphe), une valeur
//! touchée par `retain`/`release` pendant la collecte, et tout ce qu'une
//! valeur vivante atteint. Le reste ne se référence qu'entre soi : aucun
//! thread ne peut plus l'atteindre (un thread n'atteint une valeur que par
//! une référence comptée), il est libéré.
//!
//! Sûreté mémoire : pendant la collecte (`rc::EPOCH`), toute libération
//! faite par un autre thread est différée, et les éléments des conteneurs
//! sont lus sous leur verrou (`containers`).

use std::collections::HashMap;
use std::sync::atomic::Ordering;

use crate::rc::{cyclic_children, dealloc_now, finalize_object, for_each_child, kind, release, unbuffer, touched_in, word, Kind, COUNT_MASK, DEAD};

struct Node {
    val: i64,
    kind: Kind,
    count: i64,
    internal: i64,
    children: Vec<usize>,
    live: bool,
    root: bool,
}

struct Graph {
    nodes: Vec<Node>,
    index: HashMap<i64, usize>,
    epoch: i64,
}

impl Graph {
    fn node(&mut self, val: i64, kind: Kind) -> usize {
        if let Some(&i) = self.index.get(&val) {
            return i;
        }
        // `SeqCst` : voit toute opération de compte faite sans marquage (voir `rc::touch`).
        let count = unsafe { word(val, kind) }.load(Ordering::SeqCst) & COUNT_MASK;
        self.nodes.push(Node { val, kind, count, internal: 0, children: Vec::new(), live: false, root: false });
        self.index.insert(val, self.nodes.len() - 1);
        self.nodes.len() - 1
    }

    /// Relève les arêtes du sous-graphe depuis `roots`. Une valeur à compte
    /// nul est en cours de destruction par un autre thread : vivante (son
    /// bloc ne nous appartient pas), non parcourue.
    unsafe fn trace(&mut self, roots: &[usize]) {
        let mut stack = roots.to_vec();
        let mut seen = vec![false; self.nodes.len()];
        while let Some(i) = stack.pop() {
            if i >= seen.len() {
                seen.resize(self.nodes.len(), false);
            }
            if seen[i] {
                continue;
            }
            seen[i] = true;
            if self.nodes[i].count == 0 {
                self.nodes[i].live = true;
                continue;
            }
            let (val, kind) = (self.nodes[i].val, self.nodes[i].kind);
            for (t, tk) in unsafe { cyclic_children(val, kind) } {
                let j = self.node(t, tk);
                self.nodes[j].internal += 1;
                self.nodes[i].children.push(j);
                stack.push(j);
            }
        }
    }

    fn touched(&self, i: usize) -> bool {
        let node = &self.nodes[i];
        touched_in(unsafe { word(node.val, node.kind) }, self.epoch)
    }

    /// Marque vivant tout ce qu'atteignent les nœuds de `from`.
    fn spread(&mut self, from: Vec<usize>) {
        let mut stack = from;
        while let Some(i) = stack.pop() {
            for c in self.nodes[i].children.clone() {
                if !self.nodes[c].live {
                    self.nodes[c].live = true;
                    stack.push(c);
                }
            }
        }
    }

    /// Vivants : référence externe ou valeur touchée, puis tout ce qu'ils
    /// atteignent — jusqu'à ce qu'aucun nœud restant ne soit touché.
    fn mark_live(&mut self) {
        let mut fresh = Vec::new();
        for i in 0..self.nodes.len() {
            let node = &self.nodes[i];
            if node.live || node.count > node.internal || self.touched(i) {
                self.nodes[i].live = true;
                fresh.push(i);
            }
        }
        while !fresh.is_empty() {
            self.spread(fresh);
            fresh = (0..self.nodes.len()).filter(|&i| !self.nodes[i].live && self.touched(i)).collect();
            for &i in &fresh {
                self.nodes[i].live = true;
            }
        }
    }

    fn is_garbage(&self, val: i64) -> bool {
        self.index.get(&val).is_some_and(|&i| !self.nodes[i].live)
    }

    /// Ferme et rend les références sortantes des nœuds morts, puis libère
    /// leurs blocs. Les arêtes entre nœuds morts ne sont pas rendues : leurs
    /// blocs disparaissent ensemble.
    unsafe fn free_garbage(&self) {
        let garbage: Vec<&Node> = self.nodes.iter().filter(|n| !n.live).collect();
        for node in &garbage {
            if node.kind == Kind::Object {
                unsafe { finalize_object(node.val) }
            }
            unsafe {
                for_each_child(node.val, node.kind, &mut |child| {
                    if !self.is_garbage(child) {
                        release(child);
                    }
                });
            }
        }
        for node in garbage {
            unsafe { dealloc_now(node.val, node.kind) }
        }
    }
}

/// Collecte d'époque `epoch` depuis les racines `roots` (retirées du
/// tampon) ; rend celles à
/// y remettre : racines vivantes touchées pendant la collecte, dont le
/// `release` n'a pas pu les réinscrire (elles y étaient déjà).
pub(crate) unsafe fn collect(roots: Vec<i64>, epoch: i64) -> Vec<i64> {
    let mut graph = Graph { nodes: Vec::new(), index: HashMap::new(), epoch };
    let mut starts = Vec::new();
    for s in roots {
        let Some(k) = (unsafe { kind(s) }) else { continue };
        if unsafe { word(s, k) }.load(Ordering::SeqCst) & DEAD != 0 {
            unsafe { unbuffer(s, k) }
            continue;
        }
        let i = graph.node(s, k);
        graph.nodes[i].root = true;
        starts.push(i);
    }
    unsafe { graph.trace(&starts) }
    graph.mark_live();
    unsafe { graph.free_garbage() }

    // Une racine vivante reste inscrite : son `release` l'a inscrite AVANT
    // de rendre sa référence (`rc::decrement`), la décrémentation peut donc
    // encore être en cours — la retirer perdrait le cycle qu'elle forme
    // peut-être déjà. Seule une racine détruite (`DEAD`) quitte le tampon.
    let mut kept = Vec::new();
    for node in graph.nodes.iter().filter(|n| n.live && n.root) {
        if unsafe { word(node.val, node.kind) }.load(Ordering::SeqCst) & DEAD != 0 {
            unsafe { unbuffer(node.val, node.kind) }
        } else {
            kept.push(node.val);
        }
    }
    kept
}
