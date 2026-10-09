//! Per-language tree-sitter specs: which nodes are functions, and which nodes
//! are decision points (cyclomatic complexity +1).

use std::path::Path;
use tree_sitter::Language;

/// Every supported grammar spells its logical-and/or node `binary_expression`.
const OPERATOR_KIND: &str = "binary_expression";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Ts,
    Tsx,
    Rust,
    Go,
    Lua,
}

pub struct Spec {
    /// Node kinds that define a function, and therefore get their own score.
    pub functions: &'static [&'static str],
    /// Node kinds that each contribute one decision point.
    pub branches: &'static [&'static str],
    /// Operators of `binary_expression` that each contribute one decision point.
    pub ops: &'static [&'static str],
}

const TS_FUNCTIONS: &[&str] = &[
    "function_declaration",
    "generator_function_declaration",
    "function_expression",
    "arrow_function",
    "method_definition",
    "method_signature",
    "abstract_method_signature",
    "function_signature",
    "generator_function",
];
const TS_BRANCHES: &[&str] = &[
    "if_statement",
    "for_statement",
    "for_in_statement",
    "while_statement",
    "do_statement",
    "switch_case",
    "catch_clause",
    "ternary_expression",
];
const TS_OPS: &[&str] = &["&&", "||", "??"];
const TS_SPEC: Spec = Spec {
    functions: TS_FUNCTIONS,
    branches: TS_BRANCHES,
    ops: TS_OPS,
};

const RUST_FUNCTIONS: &[&str] = &["function_item", "closure_expression"];
const RUST_BRANCHES: &[&str] = &[
    "if_expression",
    "while_expression",
    "for_expression",
    "loop_expression",
    "match_arm",
    "try_expression",
];
const RUST_OPS: &[&str] = &["&&", "||"];
const RUST_SPEC: Spec = Spec {
    functions: RUST_FUNCTIONS,
    branches: RUST_BRANCHES,
    ops: RUST_OPS,
};

const GO_FUNCTIONS: &[&str] = &["function_declaration", "method_declaration", "func_literal"];
const GO_BRANCHES: &[&str] = &[
    "if_statement",
    "for_statement",
    "expression_case",
    "type_case",
    "communication_case",
];
const GO_OPS: &[&str] = &["&&", "||"];
const GO_SPEC: Spec = Spec {
    functions: GO_FUNCTIONS,
    branches: GO_BRANCHES,
    ops: GO_OPS,
};

const LUA_FUNCTIONS: &[&str] = &["function_declaration", "function_definition"];
const LUA_BRANCHES: &[&str] = &[
    "if_statement",
    "elseif_statement",
    "while_statement",
    "repeat_statement",
    "for_statement",
];
const LUA_OPS: &[&str] = &["and", "or"];
const LUA_SPEC: Spec = Spec {
    functions: LUA_FUNCTIONS,
    branches: LUA_BRANCHES,
    ops: LUA_OPS,
};

impl Lang {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "ts" | "mts" | "cts" => Some(Lang::Ts),
            "tsx" | "jsx" | "js" | "mjs" | "cjs" => Some(Lang::Tsx),
            "rs" => Some(Lang::Rust),
            "go" => Some(Lang::Go),
            "lua" => Some(Lang::Lua),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Lang::Ts => "typescript",
            Lang::Tsx => "typescript/jsx",
            Lang::Rust => "rust",
            Lang::Go => "go",
            Lang::Lua => "lua",
        }
    }

    pub fn grammar(self) -> Language {
        match self {
            Lang::Ts => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
            Lang::Go => tree_sitter_go::LANGUAGE.into(),
            Lang::Lua => tree_sitter_lua::LANGUAGE.into(),
        }
    }

    pub fn spec(self) -> &'static Spec {
        match self {
            Lang::Ts | Lang::Tsx => &TS_SPEC,
            Lang::Rust => &RUST_SPEC,
            Lang::Go => &GO_SPEC,
            Lang::Lua => &LUA_SPEC,
        }
    }

    pub fn is_function(self, kind: &str) -> bool {
        self.spec().functions.contains(&kind)
    }
}

pub fn is_operator(node_kind: &str) -> bool {
    node_kind == OPERATOR_KIND
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    /// The one check that catches a grammar/runtime ABI mismatch early.
    #[test]
    fn every_grammar_loads_and_parses() {
        for lang in [Lang::Ts, Lang::Tsx, Lang::Rust, Lang::Go, Lang::Lua] {
            let mut parser = Parser::new();
            parser
                .set_language(&lang.grammar())
                .unwrap_or_else(|e| panic!("{} failed to load: {e}", lang.name()));
            let tree = parser.parse("", None).expect("parse");
            assert_eq!(tree.root_node().kind(), root_kind(lang));
        }
    }

    fn root_kind(lang: Lang) -> &'static str {
        match lang {
            Lang::Ts | Lang::Tsx => "program",
            Lang::Rust | Lang::Go => "source_file",
            Lang::Lua => "chunk",
        }
    }

    #[test]
    fn extension_mapping() {
        assert_eq!(Lang::from_path(Path::new("a/b.ts")), Some(Lang::Ts));
        assert_eq!(Lang::from_path(Path::new("a/b.tsx")), Some(Lang::Tsx));
        assert_eq!(Lang::from_path(Path::new("a/b.rs")), Some(Lang::Rust));
        assert_eq!(Lang::from_path(Path::new("a/b.go")), Some(Lang::Go));
        assert_eq!(Lang::from_path(Path::new("a/b.lua")), Some(Lang::Lua));
        assert_eq!(Lang::from_path(Path::new("a/b.py")), None);
    }
}
