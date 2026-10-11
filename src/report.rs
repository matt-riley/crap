//! Output rendering. The JSON contract is deliberately terse and hard-bounded:
//! a model reading this output must be able to trust that its context window
//! was not quietly filled with findings it did not ask for.

use serde::Serialize;

/// Room reserved for the `dropped` object and the closing brace, so the
/// budget check can never produce truncated JSON.
const TAIL_RESERVE: usize = 160;

#[derive(Debug, Serialize)]
pub struct Finding {
    pub file: String,
    pub line: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sig: Option<String>,
    pub cc: u32,
    pub cov: Option<f64>,
    pub crap: f64,
    /// The score this function had in `--baseline`, when it was found there.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<f64>,
    /// New over the threshold, or worse than its baseline score.
    #[serde(skip_serializing_if = "is_false")]
    pub regressed: bool,
    pub sev: &'static str,
}

#[derive(Debug, Serialize)]
pub struct CovInfo {
    pub source: String,
    pub format: &'static str,
    pub files_matched: usize,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub files: usize,
    pub functions: usize,
    pub offenders: usize,
    pub worst: f64,
    pub mean: f64,
    pub crap_load: u32,
    pub threshold: f64,
    /// Offenders that are new or worse than in `--baseline`. Omitted when zero.
    #[serde(skip_serializing_if = "is_zero")]
    pub regressed: usize,
    /// Files tree-sitter could not fully parse. Omitted when zero, because a
    /// silent zero here is indistinguishable from clean code.
    #[serde(skip_serializing_if = "is_zero")]
    pub unparsed: usize,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Serialize)]
pub struct DiffInfo {
    /// Revision diffed against, or the path the patch was read from.
    pub source: String,
    /// Files the diff touches.
    pub files: usize,
    /// Files the diff touches that were also scanned and scored.
    pub matched_files: usize,
    /// Functions intersecting the diff.
    pub functions: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct Dropped {
    pub below_threshold: usize,
    pub not_shown: usize,
    /// Functions the diff did not touch, withheld because --diff was given.
    #[serde(skip_serializing_if = "is_zero")]
    pub unchanged: usize,
}

pub struct Report<'a> {
    pub ok: bool,
    pub coverage: Option<CovInfo>,
    pub diff: Option<DiffInfo>,
    pub summary: Summary,
    /// Sorted worst-first, already filtered by the threshold.
    pub findings: &'a [Finding],
    pub dropped: Dropped,
}

impl Report<'_> {
    pub fn render_json(&self, max_tokens: usize, top: usize) -> String {
        let budget = max_tokens.saturating_mul(4);

        let mut head = String::from("{\"v\":1,\"ok\":");
        head.push_str(if self.ok { "true" } else { "false" });
        head.push_str(",\"coverage\":");
        head.push_str(&match &self.coverage {
            Some(c) => serde_json::to_string(c).expect("coverage serialises"),
            None => "null".to_string(),
        });
        head.push_str(",\"summary\":");
        head.push_str(&serde_json::to_string(&self.summary).expect("summary serialises"));
        if let Some(diff) = &self.diff {
            head.push_str(",\"diff\":");
            head.push_str(&serde_json::to_string(diff).expect("diff serialises"));
        }

        let mut body = String::new();
        let mut shown = 0usize;
        for finding in self.findings.iter().take(top) {
            let one = serde_json::to_string(finding).expect("finding serialises");
            let cost = one.len() + usize::from(shown > 0);
            if max_tokens > 0 && head.len() + body.len() + cost + TAIL_RESERVE > budget {
                break;
            }
            if shown > 0 {
                body.push(',');
            }
            body.push_str(&one);
            shown += 1;
        }

        let dropped = Dropped {
            below_threshold: self.dropped.below_threshold,
            not_shown: self.findings.len() - shown,
            unchanged: self.dropped.unchanged,
        };
        format!(
            "{head},\"findings\":[{body}],\"dropped\":{}}}",
            serde_json::to_string(&dropped).expect("dropped serialises")
        )
    }

    pub fn render_text(&self, top: usize) -> String {
        let mut out = String::new();
        let shown = self.findings.iter().take(top).count();
        for f in self.findings.iter().take(top) {
            let cov = match f.cov {
                Some(c) => format!("{c:5.1}%"),
                None => "    ?".to_string(),
            };
            let scope = f
                .scope
                .as_deref()
                .map(|s| format!(" ({s})"))
                .unwrap_or_default();
            let change = match (f.regressed, f.before) {
                (true, Some(before)) => format!("  regressed from {before:.1}"),
                (true, None) => "  new offender".to_string(),
                _ => String::new(),
            };
            out.push_str(&format!(
                "{:7.1}  cc {:>3}  cov {}  {}:{}  {}{}{}\n",
                f.crap, f.cc, cov, f.file, f.line, f.name, scope, change
            ));
        }
        out.push_str(&format!(
            "{} file(s), {} function(s), {} above CRAP {}, worst {:.1}, mean {:.1}, crap load {}\n",
            self.summary.files,
            self.summary.functions,
            self.summary.offenders,
            self.summary.threshold,
            self.summary.worst,
            self.summary.mean,
            self.summary.crap_load,
        ));
        if let Some(diff) = &self.diff {
            out.push_str(&format!(
                "scoring {} function(s) touched by {} ({} changed file(s) matched)\n",
                diff.functions, diff.source, diff.matched_files,
            ));
        }
        if self.dropped.unchanged > 0 {
            out.push_str(&format!(
                "{} untouched function(s) withheld\n",
                self.dropped.unchanged
            ));
        }
        if self.dropped.below_threshold > 0 {
            out.push_str(&format!(
                "{} function(s) below the threshold withheld\n",
                self.dropped.below_threshold
            ));
        }
        if self.findings.len() > shown {
            out.push_str(&format!(
                "{} finding(s) above the threshold withheld by --top\n",
                self.findings.len() - shown
            ));
        }
        out
    }

    /// GitHub Actions workflow annotations, so CI turns findings into PR
    /// annotations rather than log lines.
    pub fn render_github(&self, top: usize) -> String {
        let mut out = String::new();
        let shown = self.findings.iter().take(top).count();
        for f in self.findings.iter().take(top) {
            let level = match f.sev {
                "crit" => "error",
                "warn" => "warning",
                _ => "notice",
            };
            let title = match &f.scope {
                Some(scope) => format!("{scope}.{}", f.name),
                None => f.name.clone(),
            };
            let cov = match f.cov {
                Some(cov) => format!("{cov:.1}%"),
                None => "?".to_string(),
            };
            let change = if f.regressed { " (regressed)" } else { "" };
            out.push_str(&format!(
                "::{level} file={},line={},title={}::CRAP {:.1} (cc {}, {cov} covered){change}\n",
                escape_property(&f.file),
                f.line,
                escape_property(&title),
                f.crap,
                f.cc,
            ));
        }
        if self.findings.len() > shown {
            out.push_str(&format!(
                "{} finding(s) above the threshold withheld by --top\n",
                self.findings.len() - shown
            ));
        }
        out
    }
}

/// Annotation properties cannot carry `%`, `:`, `,` or newlines.
fn escape_property(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
        .replace(':', "%3A")
        .replace(',', "%2C")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(findings: Vec<Finding>) -> Report<'static> {
        let leak: &'static [Finding] = Box::leak(findings.into_boxed_slice());
        Report {
            ok: false,
            coverage: None,
            diff: None,
            summary: Summary {
                files: 1,
                functions: 100,
                offenders: 50,
                worst: 156.0,
                mean: 4.0,
                crap_load: 12,
                threshold: 30.0,
                unparsed: 0,
                regressed: 0,
            },
            findings: leak,
            dropped: Dropped {
                below_threshold: 50,
                not_shown: 0,
                unchanged: 0,
            },
        }
    }

    fn findings(n: usize) -> Vec<Finding> {
        (0..n)
            .map(|i| Finding {
                file: format!("src/some/deeply/nested/module_{i}/index.ts"),
                line: i as u32 + 1,
                name: format!("function_name_number_{i}"),
                scope: Some("impl SomethingLong".to_string()),
                sig: None,
                cc: 12,
                cov: Some(0.0),
                crap: 156.0,
                before: None,
                regressed: false,
                sev: "crit",
            })
            .collect()
    }

    #[test]
    fn budget_is_a_ceiling_not_a_hint() {
        let rendered = report(findings(200)).render_json(120, 200);
        assert!(
            rendered.len().div_ceil(4) <= 120,
            "budget blown: {} chars",
            rendered.len()
        );
        assert!(rendered.contains("\"not_shown\":"));
        assert!(serde_json::from_str::<serde_json::Value>(&rendered).is_ok());
    }

    #[test]
    fn top_caps_output_even_when_the_budget_is_generous() {
        let rendered = report(findings(50)).render_json(100_000, 5);
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(parsed["findings"].as_array().unwrap().len(), 5);
        assert_eq!(parsed["dropped"]["not_shown"], 45);
        assert_eq!(parsed["dropped"]["below_threshold"], 50);
    }

    #[test]
    fn text_respects_top_and_says_what_it_hid() {
        let out = report(findings(10)).render_text(3);
        assert_eq!(out.lines().filter(|l| l.contains("cc ")).count(), 3);
        assert!(out.contains("7 finding(s) above the threshold withheld by --top"));
    }

    #[test]
    fn text_marks_a_regression_against_its_baseline() {
        let mut items = findings(1);
        items[0].before = Some(40.0);
        items[0].regressed = true;
        let out = report(items).render_text(10);
        assert!(out.contains("regressed from 40.0"), "{out}");

        let mut fresh = findings(1);
        fresh[0].regressed = true;
        assert!(report(fresh).render_text(10).contains("new offender"));
    }

    #[test]
    fn github_annotations_map_severity_and_escape_properties() {
        let mut items = findings(2);
        items[0].sev = "crit";
        items[0].name = "a,b:100%".to_string();
        items[1].sev = "ok";
        let out = report(items).render_github(10);
        assert!(out.contains("::error file="), "{out}");
        assert!(out.contains("::notice file="), "{out}");
        assert!(out.contains("a%2Cb%3A100%25"), "{out}");
    }

    #[test]
    fn tiny_budgets_still_produce_valid_json() {
        for budget in [1, 10, 50, 100] {
            let rendered = report(findings(20)).render_json(budget, 20);
            assert!(
                serde_json::from_str::<serde_json::Value>(&rendered).is_ok(),
                "budget {budget} produced invalid JSON"
            );
        }
    }
}
