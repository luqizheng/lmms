use std::collections::HashMap;

pub fn render_template(template: &str, vars: &HashMap<&'static str, String>) -> String {
    let mut out = template.to_string();
    for (k, v) in vars {
        let patterns = [format!("{{{{{}}}}}", k), format!("{{{{ {} }}}}", k)];
        for p in &patterns {
            out = out.replace(p, v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn replaces_known_placeholders() {
        let mut vars = HashMap::new();
        vars.insert("username", "Alice".to_string());
        vars.insert("dept_name", "Eng".to_string());
        vars.insert("created_at", "2026-06-27".to_string());
        let out = render_template(
            "Hi {{username}}, welcome to {{ dept_name }} on {{created_at}}.",
            &vars,
        );
        assert_eq!(out, "Hi Alice, welcome to Eng on 2026-06-27.");
    }

    #[test]
    fn leaves_unknown_placeholders_intact() {
        let vars = HashMap::new();
        let out = render_template("Hello {{username}}", &vars);
        assert_eq!(out, "Hello {{username}}");
    }

    #[test]
    fn repeated_placeholders_all_replaced() {
        let mut vars = HashMap::new();
        vars.insert("username", "Bob".to_string());
        let out = render_template("{{username}} and {{username}}", &vars);
        assert_eq!(out, "Bob and Bob");
    }
}
