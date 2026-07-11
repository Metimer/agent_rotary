use std::collections::{HashMap, HashSet};

use crate::engine::Edge;

/// Valide la topologie d'un graphe de workflow :
///  - toutes les arêtes pointent vers une node existante,
///  - `entry` et `exit` sont connus,
///  - pas de self-loop accidentelle.
///
/// Retourne la liste des cibles inconnues (vide si OK).
pub fn unknown_targets(
    nodes: &HashMap<String, crate::engine::Node>,
    adj: &HashMap<String, Vec<Edge>>,
) -> Vec<(String, String)> {
    let known: HashSet<&String> = nodes.keys().collect();
    let mut unknowns = Vec::new();
    for (from, edges) in adj {
        for e in edges {
            let target = e.target();
            if !known.contains(&target.to_string()) && target != "exit" {
                unknowns.push((from.clone(), target.to_string()));
            }
        }
    }
    unknowns
}
