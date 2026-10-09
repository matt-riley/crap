//! Changed-line detection for feature work: score only the functions a branch
//! touched, so a diff against `main` answers "is *my* code risky?".
//!
//! A function is considered changed when any line added by the diff falls
//! inside it. Deleted lines have no position in the new file, so deleting code
//! from inside a function does not by itself flag it.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Every line, for files that are new in their entirety.
const WHOLE_FILE: (u32, u32) = (1, u32::MAX);

#[derive(Debug, Default, Clone)]
pub struct Changed {
    ranges: HashMap<String, Vec<(u32, u32)>>,
    pub files: usize,
    pub hunks: usize,
}

impl Changed {
    /// Diff the working tree against `rev` using git, including untracked files
    /// — new files are the most likely thing to be written during feature work
    /// and `git diff` alone does not show them.
    pub fn from_git(rev: &str, dir: &Path) -> Result<Self, String> {
        let output = git(
            // No --relative: git's default is repo-root-relative, which is what
            // the scanned paths are too. --relative rewrites the a/ and b/
            // prefixes to c/ and w/ and loses us the exact match.
            &["diff", "--no-color", "--unified=0", rev],
            dir,
        )?;
        let mut changed = Self::parse(&String::from_utf8_lossy(&output.stdout));
        changed.add_untracked(dir)?;
        Ok(changed)
    }

    /// Ask git which files it is not tracking yet and count them as new.
    fn add_untracked(&mut self, dir: &Path) -> Result<(), String> {
        let output = git(&["ls-files", "--others", "--exclude-standard", "-z"], dir)?;
        for raw in output.stdout.split(|b| *b == 0).filter(|s| !s.is_empty()) {
            let path = normalize(&String::from_utf8_lossy(raw));
            if self.ranges.insert(path, vec![WHOLE_FILE]).is_none() {
                self.files += 1;
            }
        }
        Ok(())
    }

    /// Parse a unified diff in the format `git diff` produces.
    pub fn parse(text: &str) -> Self {
        let mut changed = Self {
            ranges: HashMap::new(),
            files: 0,
            hunks: 0,
        };
        let mut current: Option<String> = None;
        let mut open: Option<(u32, u32)> = None;
        let mut line = 0u32;
        let mut in_hunk = false;

        for raw in text.lines() {
            if raw.starts_with("@@") {
                flush(&mut changed.ranges, &mut open, &current);
                in_hunk = true;
                line = hunk_start(raw).unwrap_or(0);
                changed.hunks += 1;
                continue;
            }
            if let Some(rest) = raw.strip_prefix("+++ ") {
                flush(&mut changed.ranges, &mut open, &current);
                in_hunk = false;
                current = new_path(rest);
                changed.files += usize::from(current.is_some());
                continue;
            }
            if raw.starts_with("--- ") || raw.starts_with("diff ") || raw.starts_with("index ") {
                flush(&mut changed.ranges, &mut open, &current);
                in_hunk = false;
                continue;
            }
            if !in_hunk {
                continue;
            }
            match raw.as_bytes().first() {
                // An added line: grow the open range or start a new one.
                Some(b'+') => {
                    open = match open {
                        Some((start, end)) => Some((start, end + 1)),
                        None => Some((line, line)),
                    };
                    line += 1;
                }
                // A deletion occupies no line in the new file, so it neither
                // advances the counter nor breaks a run of additions.
                Some(b'-') | Some(b'\\') => {}
                // Context, including a blank line, advances the new-file count
                // and therefore ends any run of additions.
                Some(b' ') | None => {
                    flush(&mut changed.ranges, &mut open, &current);
                    line += 1;
                }
                // Anything else is a git extended header line, not hunk body.
                _ => {
                    flush(&mut changed.ranges, &mut open, &current);
                    in_hunk = false;
                }
            }
        }
        flush(&mut changed.ranges, &mut open, &current);
        changed
    }

    /// Added-line ranges for a scanned file, matching exactly first and then
    /// falling back to a path suffix so diffs produced from another root still
    /// line up.
    pub fn ranges_for(&self, path: &str) -> Option<&Vec<(u32, u32)>> {
        let path = normalize(path);
        if let Some(ranges) = self.ranges.get(&path) {
            return Some(ranges);
        }
        self.ranges
            .iter()
            .filter(|(key, _)| path.ends_with(key.as_str()) || key.ends_with(&path))
            .max_by_key(|(key, _)| key.len())
            .map(|(_, ranges)| ranges)
    }
}

fn git(args: &[&str], dir: &Path) -> Result<std::process::Output, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        // Git answers some failures with its entire usage text. Quote one line
        // and no more, or a mistyped rev floods the caller's context window.
        let stderr = String::from_utf8_lossy(&output.stderr);
        let first = stderr
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("failed");
        let hint = if stderr.to_lowercase().contains("not a git repository") {
            " (use --diff-file, or run inside a repository)"
        } else {
            ""
        };
        return Err(format!(
            "git {} failed: {}{hint}",
            args.join(" "),
            truncate(first, 200)
        ));
    }
    Ok(output)
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let cut = (0..max)
        .rev()
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(0);
    format!("{}…", &text[..cut])
}

pub fn overlaps(ranges: &[(u32, u32)], start: u32, end: u32) -> bool {
    ranges
        .iter()
        .any(|(low, high)| *low <= end && start <= *high)
}

fn flush(
    ranges: &mut HashMap<String, Vec<(u32, u32)>>,
    open: &mut Option<(u32, u32)>,
    current: &Option<String>,
) {
    if let (Some(range), Some(path)) = (open.take(), current) {
        ranges.entry(path.clone()).or_default().push(range);
    }
}

fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/");
    let path = path.strip_prefix("./").unwrap_or(&path);
    path.to_string()
}

/// `+++ b/src/foo.ts` -> `src/foo.ts`; `/dev/null` for a deletion.
fn new_path(rest: &str) -> Option<String> {
    let path = rest.split('\t').next().unwrap_or(rest).trim();
    let path = path.trim_matches('"');
    if path == "/dev/null" {
        return None;
    }
    Some(normalize(strip_prefix(path)))
}

/// Diff prefixes: `a/` and `b/` by default, `c/` and `w/` when the diff was
/// produced with `--relative`, `i/` and `o/` when a tool asks for those.
fn strip_prefix(path: &str) -> &str {
    match path.as_bytes() {
        [b'a' | b'b' | b'c' | b'w' | b'i' | b'o', b'/', ..] => &path[2..],
        _ => path,
    }
}

/// `@@ -1,3 +4,7 @@` -> 4
fn hunk_start(header: &str) -> Option<u32> {
    let rest = header.strip_prefix("@@ -")?;
    let after = &rest[rest.find('+')? + 1..];
    let end = after.find([' ', ',']).unwrap_or(after.len());
    after[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "\
diff --git a/src/a.rs b/src/a.rs
index 111..222 100644
--- a/src/a.rs
+++ b/src/a.rs
@@ -1,3 +1,4 @@
 keep
+added at two
 keep
 keep
@@ -10,2 +11,2 @@
 keep
-old
+added at twelve
diff --git a/src/gone.rs b/src/gone.rs
deleted file mode 100644
--- a/src/gone.rs
+++ /dev/null
@@ -1,2 +0,0 @@
-a
-b
diff --git a/src/new.rs b/src/new.rs
new file mode 100644
--- /dev/null
+++ b/src/new.rs
@@ -0,0 +1,3 @@
+one
+two
+three
";

    #[test]
    fn added_lines_become_ranges() {
        let changed = Changed::parse(DIFF);
        assert_eq!(changed.files, 2, "the deletion must not claim a file");
        // two hunks in a.rs, the deleted file, and the new file
        assert_eq!(changed.hunks, 4);
        assert_eq!(
            changed.ranges_for("src/a.rs").unwrap(),
            &vec![(2, 2), (12, 12)]
        );
        assert_eq!(changed.ranges_for("src/new.rs").unwrap(), &vec![(1, 3)]);
    }

    #[test]
    fn deleted_files_are_not_reported() {
        assert!(Changed::parse(DIFF).ranges_for("src/gone.rs").is_none());
    }

    #[test]
    fn containment_is_inclusive_at_both_ends() {
        let changed = Changed::parse(DIFF);
        let ranges = changed.ranges_for("src/a.rs").unwrap();
        assert!(!overlaps(ranges, 1, 1));
        assert!(overlaps(ranges, 1, 2));
        assert!(overlaps(ranges, 12, 12));
        assert!(overlaps(ranges, 2, 11));
        assert!(!overlaps(ranges, 13, 20));
    }

    #[test]
    fn a_path_from_another_root_still_matches() {
        let changed = Changed::parse(DIFF);
        assert!(changed.ranges_for("app/src/a.rs").is_some());
    }

    #[test]
    fn adjacent_additions_become_one_range() {
        let changed = Changed::parse("--- a/x.ts\n+++ b/x.ts\n@@ -0,0 +1,4 @@\n+a\n+b\n+c\n+d\n");
        assert_eq!(changed.ranges_for("x.ts").unwrap(), &vec![(1, 4)]);
    }

    #[test]
    fn context_lines_split_runs() {
        let changed = Changed::parse("+++ b/x.ts\n@@ -1,3 +1,3 @@\n+a\n ctx\n+b\n");
        assert_eq!(changed.ranges_for("x.ts").unwrap(), &vec![(1, 1), (3, 3)]);
    }

    #[test]
    fn plain_untabbed_diff_paths_work() {
        let changed = Changed::parse("+++ src/x.ts\t2024-01-01 00:00:00\n@@ -0,0 +1 @@\n+a\n");
        assert_eq!(changed.ranges_for("src/x.ts").unwrap(), &vec![(1, 1)]);
    }

    /// `git diff --relative` uses c/ and w/ instead of a/ and b/.
    #[test]
    fn relative_style_prefixes_are_stripped() {
        let changed = Changed::parse("--- c/src/x.ts\n+++ w/src/x.ts\n@@ -1,0 +2 @@\n+a\n");
        assert_eq!(changed.ranges_for("src/x.ts").unwrap(), &vec![(2, 2)]);
    }

    #[test]
    fn hunk_headers_without_counts_parse() {
        assert_eq!(hunk_start("@@ -1 +4 @@"), Some(4));
        assert_eq!(hunk_start("@@ -1,3 +4,7 @@"), Some(4));
        assert_eq!(hunk_start("garbage"), None);
    }
}
