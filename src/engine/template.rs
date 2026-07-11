use crate::context::Context;
use crate::error::OrchestratorResult;

/// Rend un template à doubles accolades `{{key}}` en substituant les valeurs
/// depuis le `Context`. Les sous-chaînes non trouvées sont laissées telles quelles.
/// Supporte une syntaxe fallback `{{key|default}}`.
pub fn render(template: &str, ctx: &Context) -> OrchestratorResult<String> {
    let snapshot = ctx.snapshot();
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(close) = find_close(&template[i + 2..]) {
                let expr = &template[i + 2..i + 2 + close];
                let (key, default) = match expr.find('|') {
                    Some(p) => (expr[..p].trim(), Some(expr[p + 1..].trim().to_string())),
                    None => (expr.trim(), None),
                };
                match snapshot.get(key) {
                    Some(v) if !v.is_null() => match v {
                        serde_json::Value::String(s) => out.push_str(s),
                        other => out.push_str(&other.to_string()),
                    },
                    _ => match default {
                        Some(d) => out.push_str(&d),
                        None => {
                            out.push_str("{{");
                            out.push_str(expr);
                            out.push_str("}}");
                        }
                    },
                }
                // +4 = `{{` + `}}`
                i += 2 + close + 2;
            } else {
                out.push(template.chars().next().unwrap());
                i += 1;
            }
        } else {
            let ch = template[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    Ok(out)
}

/// Trouve l'index du `}}` fermant à partir de `s`.
fn find_close(s: &str) -> Option<usize> {
    s.find("}}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_simple_value() {
        let mut ctx = Context::new();
        ctx.set("name", json!("world"));
        assert_eq!(render("hello {{name}}", &ctx).unwrap(), "hello world");
    }

    #[test]
    fn renders_with_default() {
        let ctx = Context::new();
        assert_eq!(render("v={{missing|n/a}}", &ctx).unwrap(), "v=n/a");
    }

    #[test]
    fn leaves_unknown_key_as_is() {
        let ctx = Context::new();
        assert_eq!(render("x={{unknown}}", &ctx).unwrap(), "x={{unknown}}");
    }
}
