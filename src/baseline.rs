//! Baseline comparison: score against a report captured earlier, so a run can
//! be judged by whether it made anything worse rather than by absolute score.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Only the fields needed to identify a function and remember its score. A
/// full report is accepted; unknown fields are ignored.
#[derive(Deserialize)]
struct BaselineFinding {
    file: String,
    name: String,
    #[serde(default)]
    scope: Option<String>,
    crap: f64,
}

#[derive(Deserialize)]
struct Document {
    findings: Vec<BaselineFinding>,
}

type Key = (String, Option<String>, String);

pub struct Baseline {
    scores: HashMap<Key, f64>,
}

impl Baseline {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read baseline {}: {e}", path.display()))?;
        let document: Document = serde_json::from_str(&text)
            .map_err(|e| format!("cannot parse baseline {}: {e}", path.display()))?;
        Ok(Self {
            scores: document
                .findings
                .into_iter()
                .map(|f| ((f.file, f.scope, f.name), f.crap))
                .collect(),
        })
    }

    /// The score this function had in the baseline, keyed on identity rather
    /// than line number so edits elsewhere cannot fake a regression.
    pub fn before(&self, file: &str, scope: Option<&str>, name: &str) -> Option<f64> {
        self.scores
            .get(&(
                file.to_string(),
                scope.map(str::to_string),
                name.to_string(),
            ))
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, body: &str) -> std::path::PathBuf {
        let path = dir.join("baseline.json");
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn keys_on_identity_not_line_number() {
        let dir = std::env::temp_dir().join("crap-baseline-unit");
        std::fs::create_dir_all(&dir).unwrap();
        let path = write(
            &dir,
            r#"{"v":1,"findings":[
                {"file":"src/a.rs","line":90,"name":"run","scope":"impl A","cc":12,"cov":0.0,"crap":156.0,"sev":"crit"},
                {"file":"src/b.rs","line":1,"name":"plain","crap":2.0}
            ]}"#,
        );
        let baseline = Baseline::load(&path).unwrap();
        assert_eq!(
            baseline.before("src/a.rs", Some("impl A"), "run"),
            Some(156.0)
        );
        assert_eq!(baseline.before("src/b.rs", None, "plain"), Some(2.0));
        assert_eq!(baseline.before("src/a.rs", None, "run"), None);
        assert_eq!(baseline.before("src/a.rs", Some("impl A"), "other"), None);
    }

    #[test]
    fn a_report_that_is_not_a_report_is_an_error() {
        let dir = std::env::temp_dir().join("crap-baseline-bad");
        std::fs::create_dir_all(&dir).unwrap();
        let path = write(&dir, "not json");
        assert!(Baseline::load(&path).is_err());
        assert!(Baseline::load(&dir.join("missing.json")).is_err());
    }
}
