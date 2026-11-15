//! Minimal template renderer: `{{ key }}` substitutes, `{{# comment }}` is dropped,
//! `{{? key }}...{{/?}}` keeps the block when key is "true"/non-empty and not "false"/"0",
//! `{{! key }}...{{/!}}` is the inverse. Unknown keys are errors so typos never ship.

use std::collections::BTreeMap;

pub type Vars = BTreeMap<String, String>;

fn truthy(v: Option<&String>) -> bool {
    matches!(v, Some(s) if !s.is_empty() && s != "false" && s != "0")
}

pub fn render(src: &str, vars: &Vars) -> Result<String, String> {
    let mut out = String::with_capacity(src.len());
    // Stack of (is_active) for conditional blocks.
    let mut stack: Vec<bool> = vec![];
    let active = |s: &Vec<bool>| s.iter().all(|&b| b);
    let mut rest = src;
    let mut line = 1usize;
    while let Some(start) = rest.find("{{") {
        let (before, after) = rest.split_at(start);
        if active(&stack) {
            out.push_str(before);
        }
        line += before.matches('\n').count();
        let end = after.find("}}").ok_or_else(|| format!("line {line}: unclosed '{{{{'"))?;
        let tag = after[2..end].trim();
        rest = &after[end + 2..];
        if let Some(k) = tag.strip_prefix('?') {
            let k = k.trim();
            if !vars.contains_key(k) {
                return Err(format!("line {line}: unknown variable '{k}'"));
            }
            stack.push(truthy(vars.get(k)));
        } else if let Some(k) = tag.strip_prefix('!') {
            let k = k.trim();
            if !vars.contains_key(k) {
                return Err(format!("line {line}: unknown variable '{k}'"));
            }
            stack.push(!truthy(vars.get(k)));
        } else if tag == "/?" || tag == "/!" {
            stack.pop().ok_or_else(|| format!("line {line}: unmatched '{{{{{tag}}}}}'"))?;
        } else if tag.starts_with('#') {
            // comment
        } else {
            let v = vars.get(tag).ok_or_else(|| format!("line {line}: unknown variable '{tag}'"))?;
            if active(&stack) {
                out.push_str(v);
            }
        }
    }
    if !stack.is_empty() {
        return Err("unclosed conditional block".into());
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vars {
        [("a.b", "X"), ("yes", "true"), ("no", "false")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn substitutes() {
        assert_eq!(render("x {{ a.b }} y {{a.b}}", &vars()).unwrap(), "x X y X");
        assert_eq!(render("{{# hi }}z", &vars()).unwrap(), "z");
    }

    #[test]
    fn conditionals() {
        assert_eq!(render("{{? yes }}1{{/?}}{{? no }}2{{/?}}{{! no }}3{{/!}}", &vars()).unwrap(), "13");
        assert_eq!(render("{{? yes }}{{? no }}a{{/?}}b{{/?}}", &vars()).unwrap(), "b");
    }

    #[test]
    fn errors() {
        assert!(render("{{ nope }}", &vars()).unwrap_err().contains("nope"));
        assert!(render("a\n{{ x", &vars()).unwrap_err().contains("line 2"));
        assert!(render("{{? yes }}", &vars()).is_err());
        assert!(render("{{/?}}", &vars()).is_err());
    }
}
