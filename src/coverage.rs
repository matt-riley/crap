//! Coverage ingestion: sniff the format, normalise every source into a list of
//! `(line, hits)` points, and match them onto source files by path suffix.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// Where coverage tools put their reports, most common first.
const AUTO_DISCOVERY: &[&str] = &[
    "lcov.info",
    "coverage/lcov.info",
    "target/llvm-cov/lcov.info",
    "cov.lcov",
    "coverage-final.json",
    "coverage/coverage-final.json",
    "cobertura.xml",
    "coverage.xml",
    "coverage.out",
];

pub fn auto_discover() -> Option<PathBuf> {
    AUTO_DISCOVERY
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

type Lines = Vec<(u32, u64)>;
type Files = Vec<(String, Lines)>;

pub struct Coverage {
    pub source: String,
    pub format: &'static str,
    files: Files,
    /// Trailing path suffixes that identify exactly one coverage file.
    index: HashMap<String, usize>,
}

impl Coverage {
    pub fn load(path: &Path) -> Result<Self, String> {
        Self::load_all(std::slice::from_ref(&path.to_path_buf()))
    }

    pub fn load_all(paths: &[PathBuf]) -> Result<Self, String> {
        let mut files = Files::new();
        let mut format = "lcov";
        let mut source = String::new();

        for (i, path) in paths.iter().enumerate() {
            let text = fs::read_to_string(path)
                .map_err(|e| format!("cannot read coverage {}: {e}", path.display()))?;
            let (detected, mut parsed) = parse(&text)
                .ok_or_else(|| format!("unrecognised coverage format: {}", path.display()))?;
            let label = display(path);
            if i == 0 {
                format = detected;
                source = label;
            } else {
                source = format!("{source}+{label}");
            }
            files.append(&mut parsed);
        }

        let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, (name, _)) in files.iter().enumerate() {
            for key in key_variants(name) {
                by_key.entry(key).or_default().push(i);
            }
        }
        // A suffix claimed by more than one coverage file (usually a bare
        // filename like `lib.rs`) is ambiguous and must not be guessed at.
        let index = by_key
            .into_iter()
            .filter(|(_, owners)| owners.len() == 1)
            .map(|(key, owners)| (key, owners[0]))
            .collect();

        Ok(Coverage {
            source,
            format,
            files,
            index,
        })
    }

    /// Longest suffix first, so `src/a/mod.rs` prefers `src/a/mod.rs` over
    /// `mod.rs`.
    pub fn file_lines(&self, path: &str) -> Option<&Lines> {
        key_variants(path)
            .iter()
            .find_map(|k| self.index.get(k))
            .map(|i| &self.files[*i].1)
    }

    /// Percentage of instrumented lines covered in `[start, end]`.
    /// `None` means the report says nothing about this code.
    pub fn pct(&self, path: &str, start: u32, end: u32) -> Option<f64> {
        self.file_lines(path)
            .and_then(|lines| pct_in(lines, start, end))
    }
}

pub fn pct_in(lines: &Lines, start: u32, end: u32) -> Option<f64> {
    let mut total = 0u32;
    let mut covered = 0u32;
    for (line, hits) in lines {
        if *line >= start && *line <= end {
            total += 1;
            if *hits > 0 {
                covered += 1;
            }
        }
    }
    (total > 0).then(|| f64::from(covered) / f64::from(total) * 100.0)
}

/// `a/b/c.rs` -> `["a/b/c.rs", "b/c.rs", "c.rs"]`
fn key_variants(path: &str) -> Vec<String> {
    let normalised = path.replace('\\', "/");
    let parts: Vec<&str> = normalised
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    (1..=parts.len())
        .rev()
        .map(|n| parts[parts.len() - n..].join("/"))
        .collect()
}

fn display(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    text.strip_prefix("./").unwrap_or(&text).to_string()
}

fn parse(text: &str) -> Option<(&'static str, Files)> {
    let head = text.trim_start();
    if text
        .lines()
        .any(|l| l.starts_with("SF:") || l.starts_with("TN:"))
    {
        return Some(("lcov", parse_lcov(text)));
    }
    if head.starts_with("mode: ") {
        return Some(("go coverprofile", parse_go(text)));
    }
    if head.starts_with('{') {
        return parse_istanbul(text).map(|f| ("istanbul", f));
    }
    if head.starts_with("<?xml") || head.starts_with("<coverage") {
        return Some(("cobertura", parse_cobertura(text)));
    }
    None
}

fn parse_lcov(text: &str) -> Files {
    let mut out = Files::new();
    let mut current: Option<(String, BTreeMap<u32, u64>)> = None;

    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("SF:") {
            current = Some((rest.trim().to_string(), BTreeMap::new()));
        } else if let Some(rest) = line.strip_prefix("DA:") {
            if let Some((_, map)) = current.as_mut() {
                let mut fields = rest.split(',');
                if let (Some(line), Some(hits)) = (fields.next(), fields.next())
                    && let (Ok(line), Ok(hits)) =
                        (line.trim().parse::<u32>(), hits.trim().parse::<u64>())
                {
                    *map.entry(line).or_insert(0) += hits;
                }
            }
        } else if line.starts_with("end_of_record")
            && let Some(entry) = current.take()
        {
            out.push(finish(entry));
        }
    }
    if let Some(entry) = current.take() {
        out.push(finish(entry));
    }
    out
}

/// `path/to/file.go:12.5,14.2 2 1` — one entry per basic block; the block's
/// first line stands in for the block.
fn parse_go(text: &str) -> Files {
    let mut by_file: BTreeMap<String, BTreeMap<u32, u64>> = BTreeMap::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("mode:") {
            continue;
        }
        let Some((path, rest)) = line.rsplit_once(':') else {
            continue;
        };
        let mut fields = rest.split_whitespace();
        let (Some(coords), Some(count)) = (fields.next(), fields.nth(1)) else {
            continue;
        };
        let Some((start, _)) = coords.split_once(',') else {
            continue;
        };
        let Some((start_line, _)) = start.split_once('.') else {
            continue;
        };
        let (Ok(start_line), Ok(count)) = (start_line.parse::<u32>(), count.parse::<u64>()) else {
            continue;
        };
        *by_file
            .entry(path.to_string())
            .or_default()
            .entry(start_line)
            .or_insert(0) += count;
    }

    by_file
        .into_iter()
        .map(|(p, m)| (p, m.into_iter().collect()))
        .collect()
}

fn parse_istanbul(text: &str) -> Option<Files> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let object = value.as_object()?;
    let mut out = Files::new();

    for (path, entry) in object {
        let statements = entry.get("statementMap").and_then(|v| v.as_object())?;
        let counts = entry.get("s").and_then(|v| v.as_object());
        let mut map = BTreeMap::new();
        for (id, statement) in statements {
            let Some(line) = statement.pointer("/start/line").and_then(|v| v.as_u64()) else {
                continue;
            };
            let hits = counts
                .and_then(|c| c.get(id))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            *map.entry(line as u32).or_insert(0) += hits;
        }
        if !map.is_empty() {
            out.push((path.clone(), map.into_iter().collect()));
        }
    }

    (!out.is_empty()).then_some(out)
}

fn parse_cobertura(text: &str) -> Files {
    let mut out = Files::new();
    let mut pos = 0;

    while let Some(offset) = text[pos..].find("<class") {
        let start = pos + offset;
        let end = text[start..]
            .find("</class>")
            .map_or(text.len(), |e| start + e);
        let chunk = &text[start..end];

        if let Some(filename) = attr(chunk, "filename=\"") {
            let mut map = BTreeMap::new();
            let mut cursor = 0;
            while let Some(offset) = chunk[cursor..].find("<line ") {
                cursor += offset + "<line ".len();
                let tag_end = chunk[cursor..]
                    .find('>')
                    .map_or(chunk.len(), |e| cursor + e);
                let tag = &chunk[cursor..tag_end];
                if let (Some(line), Some(hits)) =
                    (attr_u32(tag, "number=\""), attr_u64(tag, "hits=\""))
                {
                    *map.entry(line).or_insert(0) += hits;
                }
            }
            if !map.is_empty() {
                out.push((filename, map.into_iter().collect()));
            }
        }
        pos = end;
    }

    out
}

fn attr(text: &str, prefix: &str) -> Option<String> {
    let start = text.find(prefix)? + prefix.len();
    let end = text[start..].find('"')? + start;
    Some(text[start..end].to_string())
}

fn attr_u32(text: &str, prefix: &str) -> Option<u32> {
    attr(text, prefix)?.parse().ok()
}

fn attr_u64(text: &str, prefix: &str) -> Option<u64> {
    attr(text, prefix)?.parse().ok()
}

fn finish((path, map): (String, BTreeMap<u32, u64>)) -> (String, Lines) {
    (path, map.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every format describes the same fictional file: lines 2,3,4 covered,
    /// lines 6,7 not.
    const EXPECTED: f64 = 3.0 / 5.0 * 100.0;

    fn fixture(name: &str) -> Coverage {
        Coverage::load(Path::new(&format!("tests/fixtures/{name}"))).unwrap()
    }

    #[test]
    fn every_format_agrees() {
        for name in [
            "lcov.info",
            "coverage.out",
            "cobertura.xml",
            "coverage-final.json",
        ] {
            let cov = fixture(name);
            let got = cov.pct("src/sample.rs", 1, 10).unwrap_or_else(|| {
                panic!("{name} matched no lines; index keys: {:?}", cov.index.len())
            });
            assert!((got - EXPECTED).abs() < 1e-9, "{name} gave {got}");
        }
    }

    #[test]
    fn suffix_matching_handles_monorepo_prefixes() {
        let got = fixture("lcov-prefixed.info")
            .pct("src/sample.rs", 1, 10)
            .unwrap();
        assert!((got - EXPECTED).abs() < 1e-9);
    }

    #[test]
    fn ambiguous_suffixes_are_not_guessed() {
        let cov = fixture("lcov-ambiguous.info");
        assert!(cov.file_lines("src/sample.rs").is_none());
        assert!(cov.file_lines("a/sample.rs").is_some());
    }

    #[test]
    fn nothing_reported_is_not_the_same_as_zero_percent() {
        assert_eq!(fixture("lcov.info").pct("src/sample.rs", 100, 200), None);
    }

    #[test]
    fn path_keys_are_longest_first() {
        assert_eq!(
            key_variants("src/a/b.ts"),
            vec![
                "src/a/b.ts".to_string(),
                "a/b.ts".to_string(),
                "b.ts".to_string()
            ]
        );
    }
}
