//! Token-aware CRAP scoring for TypeScript, Rust, Go and Lua.

pub mod baseline;
pub mod coverage;
pub mod diff;
pub mod lang;
pub mod report;
pub mod score;

/// Portable path text: forward slashes, no leading `./`. Every module that
/// keys on paths shares this one spelling.
pub fn normalize_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    path.strip_prefix("./").unwrap_or(&path).to_string()
}

#[cfg(test)]
mod tests {
    use super::normalize_path;

    #[test]
    fn normalises_separators_and_a_leading_dot_slash() {
        assert_eq!(normalize_path("./src/a.rs"), "src/a.rs");
        assert_eq!(normalize_path(".\\src\\a.rs"), "src/a.rs");
        assert_eq!(normalize_path("src/a.rs"), "src/a.rs");
    }
}
