use serde_json::Value;
use std::collections::HashMap;

/// Sac de données dynamique partagé entre les nodes pendant l'exécution du workflow.
/// Chaque node écrit son résultat sous la clé `<node_id>.output` (et toute clé
/// arbitraire qu'elle souhaite exposer, ex. `<node_id>.score`).
#[derive(Clone, Default)]
pub struct Context {
    data: HashMap<String, Value>,
}

impl Context {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set<K: Into<String>>(&mut self, key: K, val: Value) {
        self.data.insert(key.into(), val);
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    /// Accès stringifié, pratique pour les templates `{{node.output}}`.
    pub fn get_str(&self, key: &str) -> Option<String> {
        match self.data.get(key)? {
            Value::String(s) => Some(s.clone()),
            v => Some(v.to_string()),
        }
    }

    /// Accès numérique (note, score), avec valeur par défaut.
    pub fn get_number(&self, key: &str) -> Option<f64> {
        self.data.get(key).and_then(|v| v.as_f64())
    }

    pub fn snapshot(&self) -> &HashMap<String, Value> {
        &self.data
    }

    /// Fusionne un autre contexte dans celui-ci (les clés de `other` gagnent).
    pub fn merge(&mut self, other: Context) {
        for (k, v) in other.data {
            self.data.insert(k, v);
        }
    }
}
