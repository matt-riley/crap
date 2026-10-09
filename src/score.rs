//! Function discovery and CRAP scoring.
//!
//! Cyclomatic complexity is `1 + decisions`, where a decision is a literal
//! branch site in the function's own body. Nested functions are excluded from
//! their parent's count and scored on their own row, so a file's rows are
//! additive and coverage attribution stays coherent.

use crate::lang::{Lang, Spec, is_operator};
use tree_sitter::{Node, Tree};

#[derive(Debug, Clone)]
pub struct Func {
    pub name: String,
    pub scope: Option<String>,
    /// One-line excerpt of the declaration, only when requested.
    pub sig: Option<String>,
    pub line: u32,
    pub end_line: u32,
    pub cc: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub include_tests: bool,
    pub snippet: bool,
}

/// CRAP(m) = cc² × (1 − cov/100)³ + cc
///
/// `cov` of `None` means "not measured", which is scored as zero coverage —
/// the worst case, which is what you want from a risk metric.
pub fn crap(cc: u32, cov: Option<f64>) -> f64 {
    let cc = f64::from(cc);
    let covered = cov.unwrap_or(0.0).clamp(0.0, 100.0) / 100.0;
    cc * cc * (1.0 - covered).powi(3) + cc
}

/// Severity bands scale with the threshold: `warn` above it, `crit` above
/// twice it. The defaults (30/60) reproduce the classic crap4j bands.
pub fn severity(crap: f64, threshold: f64) -> &'static str {
    if crap > threshold * 2.0 {
        "crit"
    } else if crap > threshold {
        "warn"
    } else {
        "ok"
    }
}

/// Minimum work to clear the threshold, from crap4j: one test per uncovered
/// point, plus one extract-method per threshold's worth of complexity.
pub fn crap_load(cc: u32, cov: Option<f64>, threshold: f64) -> f64 {
    let cc = f64::from(cc);
    cc * (1.0 - cov.unwrap_or(0.0).clamp(0.0, 100.0) / 100.0) + cc / threshold
}

pub fn discover(tree: &Tree, src: &[u8], lang: Lang, options: Options) -> Vec<Func> {
    let spec = lang.spec();
    let mut out = Vec::new();
    let mut stack: Vec<(Node, Option<String>)> = vec![(tree.root_node(), None)];

    while let Some((node, scope)) = stack.pop() {
        if !options.include_tests && lang == Lang::Rust && is_rust_test(node, src) {
            continue;
        }

        let mut child_scope = scope.clone();

        if lang.is_function(node.kind()) {
            let name = func_name(node, src);
            let func_scope = if lang == Lang::Go && node.kind() == "method_declaration" {
                go_receiver(node, src).or(scope)
            } else {
                scope
            };
            let display = match &func_scope {
                Some(s) => format!("{s}.{name}"),
                None => name.clone(),
            };
            out.push(Func {
                name,
                scope: func_scope,
                sig: options.snippet.then(|| signature(node, src)),
                line: node.start_position().row as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
                cc: complexity(node, src, spec),
            });
            child_scope = Some(display);
        } else if let Some(s) = scope_name(node, src, lang) {
            child_scope = Some(s);
        }

        for child in children_of(node).into_iter().rev() {
            stack.push((child, child_scope.clone()));
        }
    }

    out
}

/// Complexity of `node`'s own body: decisions inside nested functions belong to
/// those functions, not to this one.
pub fn complexity(node: Node, src: &[u8], spec: &Spec) -> u32 {
    let root_id = node.id();
    let mut cc = 1u32;
    let mut stack = vec![node];

    while let Some(n) = stack.pop() {
        if n.id() != root_id && spec.functions.contains(&n.kind()) {
            continue;
        }
        if spec.branches.contains(&n.kind()) {
            cc += 1;
        } else if is_operator(n.kind())
            && let Some(op) = n.child_by_field_name("operator")
            && spec.ops.contains(&op.utf8_text(src).unwrap_or(""))
        {
            cc += 1;
        }
        stack.extend(children_of(n));
    }

    cc
}

/// The declaration's first line, which is almost always the signature.
fn signature(node: Node, src: &[u8]) -> String {
    let start = node.start_byte();
    let end = src[start..]
        .iter()
        .position(|b| *b == b'\n')
        .map_or(node.end_byte(), |offset| start + offset);
    norm(&String::from_utf8_lossy(&src[start..end]), 100)
}

fn children_of(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

fn node_text<'a>(node: Node, src: &'a [u8]) -> &'a str {
    node.utf8_text(src).unwrap_or("")
}

/// Collapse whitespace and cap the length, so names cannot bloat the output.
fn norm(s: &str, max: usize) -> String {
    let mut out = String::with_capacity(s.len().min(max));
    let mut prev_space = false;
    let mut truncated = false;
    for ch in s.chars() {
        if out.len() >= max {
            truncated = true;
            break;
        }
        if ch.is_whitespace() {
            if !out.is_empty() && !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    let mut out = out.trim_end().to_string();
    if truncated {
        out.push('…');
    }
    out
}

/// Scopes that are not functions but still qualify names: `impl Foo`,
/// `class Service`, `mod tests`.
fn scope_name(node: Node, src: &[u8], lang: Lang) -> Option<String> {
    let kind = node.kind();
    let text = |field: &str| {
        node.child_by_field_name(field)
            .map(|n| norm(node_text(n, src), 48))
            .filter(|s| !s.is_empty())
    };
    match lang {
        Lang::Rust => match kind {
            "impl_item" => text("type").map(|t| format!("impl {t}")),
            "trait_item" | "mod_item" => text("name"),
            _ => None,
        },
        Lang::Ts | Lang::Tsx => match kind {
            "class_declaration" | "interface_declaration" => text("name"),
            _ => None,
        },
        Lang::Go | Lang::Lua => None,
    }
}

fn func_name(node: Node, src: &[u8]) -> String {
    if let Some(n) = node.child_by_field_name("name") {
        let t = norm(node_text(n, src), 64);
        if !t.is_empty() {
            return t;
        }
    }
    if let Some(parent) = node.parent()
        && let Some(name) = assigned_name(parent, src)
    {
        return name;
    }
    "<anonymous>".to_string()
}

/// `const handler = () => {}`, `x = function() end`, `let f = || {}` — recover
/// the binding the function was assigned to, but only when it is the direct
/// value (a closure passed as an argument stays anonymous).
fn assigned_name(node: Node, src: &[u8]) -> Option<String> {
    let field = match node.kind() {
        "variable_declarator"
        | "public_field_definition"
        | "field_definition"
        | "field"
        | "property_signature" => "name",
        "pair" => "key",
        "let_declaration" | "static_item" => "pattern",
        "assignment_expression" | "assignment_statement" | "short_var_declaration" => {
            // Lua spells the target `(assignment_statement (variable_list …))`
            // rather than a field; Go uses a `left` field.
            let mut cursor = node.walk();
            let target = node
                .children(&mut cursor)
                .find(|c| c.kind() == "variable_list")
                .or_else(|| node.child_by_field_name("left"))?;
            return Some(norm(node_text(target, src), 64)).filter(|s| !s.is_empty());
        }
        "expression_list" => {
            // Go wraps the bound value (`f := func(){}`) in an expression_list;
            // Lua does the same for `local f = function() end`.
            let grandparent = node.parent()?;
            return match grandparent.kind() {
                "short_var_declaration" | "assignment_statement" => assigned_name(grandparent, src),
                _ => None,
            };
        }
        _ => return None,
    };
    let named = node.child_by_field_name(field)?;
    let t = norm(node_text(named, src), 64);
    (!t.is_empty()).then_some(t)
}

fn go_receiver(node: Node, src: &[u8]) -> Option<String> {
    let recv = node.child_by_field_name("receiver")?;
    let decl = recv.named_child(0)?;
    let ty = decl.child_by_field_name("type")?;
    let t = norm(node_text(ty, src), 48);
    Some(t.trim_start_matches('*').to_string()).filter(|s| !s.is_empty())
}

/// Rust test code: `#[test]`-attributed functions and `#[cfg(test)]` modules.
/// Attributes are siblings that precede the item.
fn is_rust_test(node: Node, src: &[u8]) -> bool {
    let kind = node.kind();
    if kind != "function_item" && kind != "mod_item" && kind != "closure_expression" {
        return false;
    }
    let mut sibling = node.prev_sibling();
    while let Some(s) = sibling {
        match s.kind() {
            "attribute_item" => {
                let text = node_text(s, src);
                if text.contains("test") {
                    return true;
                }
            }
            "comment" => {}
            _ => break,
        }
        sibling = s.prev_sibling();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crap_matches_the_published_worked_examples() {
        // cc 12, 0% coverage -> 156
        assert!((crap(12, Some(0.0)) - 156.0).abs() < 1e-9);
        // 100% coverage -> complexity alone
        assert!((crap(4, Some(100.0)) - 4.0).abs() < 1e-9);
        // cc 10, 50% -> 10² × 0.5³ + 10 = 22.5
        assert!((crap(10, Some(50.0)) - 22.5).abs() < 1e-9);
        // unmeasured coverage is scored as uncovered
        assert!((crap(6, None) - 42.0).abs() < 1e-9);
    }

    #[test]
    fn crap_load_uses_the_configured_threshold() {
        // cc 60, uncovered: 60 tests + 60/30 extract-methods
        assert!((crap_load(60, Some(0.0), 30.0) - 62.0).abs() < 1e-9);
        // the same method against a stricter threshold needs more splitting
        assert!((crap_load(60, Some(0.0), 10.0) - 66.0).abs() < 1e-9);
        // half covered: 30 tests left, plus the splits
        assert!((crap_load(60, Some(50.0), 30.0) - 32.0).abs() < 1e-9);
    }

    #[test]
    fn severity_bands() {
        assert_eq!(severity(30.0, 30.0), "ok");
        assert_eq!(severity(30.1, 30.0), "warn");
        assert_eq!(severity(60.0, 30.0), "warn");
        assert_eq!(severity(60.1, 30.0), "crit");
    }

    #[test]
    fn name_normalisation() {
        assert_eq!(norm("  a\n b\t c  ", 32), "a b c");
        assert_eq!(norm("abcdef", 3), "abc…");
    }
}
