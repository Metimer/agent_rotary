use std::collections::{HashMap, HashSet};

use crate::engine::Edge;

/// Retourne les arêtes dont la cible ne correspond à aucune node.
pub fn unknown_targets(
    nodes: &HashMap<String, crate::engine::Node>,
    adj: &HashMap<String, Vec<Edge>>,
) -> Vec<(String, String)> {
    let mut unknowns = Vec::new();
    for (from, edges) in adj {
        for e in edges {
            let target = e.target();
            if !nodes.contains_key(target) && target != "exit" {
                unknowns.push((from.clone(), target.to_string()));
            }
        }
    }
    unknowns
}

/// Retourne les sources d'arêtes qui ne correspondent a aucune node.
pub fn unknown_sources(
    nodes: &HashMap<String, crate::engine::Node>,
    adj: &HashMap<String, Vec<Edge>>,
) -> Vec<String> {
    adj.keys()
        .filter(|source| !nodes.contains_key(*source))
        .cloned()
        .collect()
}

/// Verifie qu'un chemin topologique vers exit existe depuis l'entry.
pub fn can_reach_exit(entry: &str, adj: &HashMap<String, Vec<Edge>>) -> bool {
    let mut seen = HashSet::new();
    let mut pending = vec![entry];

    while let Some(current) = pending.pop() {
        if current == "exit" {
            return true;
        }
        if !seen.insert(current.to_string()) {
            continue;
        }
        if let Some(edges) = adj.get(current) {
            pending.extend(edges.iter().map(|edge| edge.target()));
        }
    }
    false
}
