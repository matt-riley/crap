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
    /// Files tree-sitter could not fully parse. Omitted when zero, because a
    /// silent zero here is indistinguishable from clean code.
    #[serde(skip_serializing_if = "is_zero")]
    pub unparsed: usize,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
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

    pub fn render_text(&self) -> String {
        let mut out = String::new();
        for f in self.findings {
            let cov = match f.cov {
                Some(c) => format!("{c:5.1}%"),
                None => "    ?".to_string(),
            };
            let scope = f
                .scope
                .as_deref()
                .map(|s| format!(" ({s})"))
                .unwrap_or_default();
            out.push_str(&format!(
                "{:7.1}  cc {:>3}  cov {}  {}:{}  {}{}\n",
                f.crap, f.cc, cov, f.file, f.line, f.name, scope
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
        out
    }
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
