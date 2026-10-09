use clap::{Parser, ValueEnum};
use crap_cli::coverage::{self, Coverage};
use crap_cli::diff::{self, Changed};
use crap_cli::lang::Lang;
use crap_cli::report::{CovInfo, DiffInfo, Dropped, Finding, Report, Summary};
use crap_cli::score::{self, Options};
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use tree_sitter::Parser as TreeSitter;

const AFTER_HELP: &str = "\
CRAP(m) = comp(m)^2 * (1 - cov(m)/100)^3 + comp(m)

comp is cyclomatic complexity (1 + branch sites). cov is the share of
instrumented lines covered inside the function. With no coverage report every
function is scored at 0% coverage, so every score is the cc^2 + cc upper bound.

Exit codes: 0 report, 1 threshold tripped with --fail-above, 2 error.

Feature work: --diff scores only the functions a branch touched. `--diff`
covers uncommitted work, `--diff main` adds it to the difference from main,
and `--diff 'main...HEAD'` is the committed work since branching. --diff-file
reads a patch instead, so CI can pass one in.";

#[derive(Parser)]
#[command(
    name = "crap",
    version,
    about = "Rank functions by Change Risk Anti-Patterns score",
    after_help = AFTER_HELP
)]
struct Cli {
    /// Files or directories to scan
    #[arg(value_name = "PATH", default_value = ".")]
    paths: Vec<PathBuf>,

    /// Coverage report; format detected from content. Repeatable.
    #[arg(short, long, value_name = "FILE")]
    coverage: Vec<PathBuf>,

    /// Ignore auto-discovered coverage and score everything at 0%
    #[arg(long)]
    no_coverage: bool,

    /// Diff the working tree against REV and score only changed functions
    #[arg(
        long,
        value_name = "REV",
        num_args = 0..=1,
        default_missing_value = "HEAD",
        conflicts_with = "diff_file"
    )]
    diff: Option<String>,

    /// Score only functions touched by a unified diff file (use - for stdin)
    #[arg(long, value_name = "PATH")]
    diff_file: Option<PathBuf>,

    /// CRAP score above which a function is reported
    #[arg(long, default_value_t = 30.0, value_name = "N")]
    threshold: f64,

    /// Maximum findings to report
    #[arg(long, default_value_t = 20, value_name = "N")]
    top: usize,

    /// Approximate token budget for JSON output (chars/4); 0 disables it
    #[arg(long, default_value_t = 1500, value_name = "N")]
    max_tokens: usize,

    /// Include functions at or below the threshold
    #[arg(long)]
    all: bool,

    /// Output format
    #[arg(long, value_enum, default_value_t = Format::Json)]
    format: Format,

    /// Include a one-line signature excerpt per finding
    #[arg(long)]
    snippet: bool,

    /// Score test files, #[cfg(test)] modules and #[test] functions too
    #[arg(long)]
    include_tests: bool,

    /// Additional gitignore-style pattern to skip. Repeatable.
    #[arg(long, value_name = "GLOB")]
    exclude: Vec<String>,

    /// Exit 1 when any function exceeds the threshold
    #[arg(long)]
    fail_above: bool,

    /// Print nothing (useful with --fail-above)
    #[arg(short, long)]
    quiet: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Json,
    Text,
    Summary,
}

/// Never scanned: build output and dependencies.
const ALWAYS_SKIP: &[&str] = &[
    "node_modules/",
    "target/",
    "dist/",
    "build/",
    "out/",
    "vendor/",
    "coverage/",
    "__pycache__/",
];

/// Skipped unless --include-tests: test code is complex by nature and is not
/// what anyone means by "risky untested logic".
const TEST_SKIP: &[&str] = &[
    "tests/",
    "test/",
    "__tests__/",
    "spec/",
    "testdata/",
    "*.test.*",
    "*.spec.*",
    "*_test.go",
];

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("crap: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: &Cli) -> Result<ExitCode, String> {
    let coverage = load_coverage(cli)?;
    let changed = load_diff(cli)?;
    let files = collect(cli)?;
    let scanned = files.len();

    let parsed: Vec<Parsed> = files
        .par_iter()
        .map_init(TreeSitter::new, |parser, file| {
            parse_file(parser, file, cli)
        })
        .collect();

    let mut findings = Vec::new();
    let mut covered_files = 0usize;
    let mut diff_files = 0usize;
    let mut diff_functions = 0usize;
    for file in &parsed {
        let lines = coverage.as_ref().and_then(|c| c.file_lines(&file.path));
        if lines.is_some() {
            covered_files += 1;
        }
        // Match the diff once per file rather than once per function.
        let ranges = changed.as_ref().and_then(|c| c.ranges_for(&file.path));
        if ranges.is_some() {
            diff_files += 1;
        }
        for func in &file.funcs {
            let cov = lines.and_then(|l| coverage::pct_in(l, func.line, func.end_line));
            let finding = to_finding(file, func, cov, cli.threshold);
            let touched = ranges.is_some_and(|r| {
                let touched = diff::overlaps(r, func.line, func.end_line);
                diff_functions += usize::from(touched);
                touched
            });
            findings.push((touched, finding));
        }
    }

    // In diff mode the report describes the changed functions; otherwise all of them.
    let in_diff_mode = changed.is_some();
    let total_functions = findings.len();
    let unchanged = if in_diff_mode {
        total_functions - diff_functions
    } else {
        0
    };
    let mut scored: Vec<Finding> = findings
        .into_iter()
        .filter(|(touched, _)| !in_diff_mode || *touched)
        .map(|(_, finding)| finding)
        .collect();

    let unparsed = parsed.iter().filter(|f| f.unparsed).count();
    let offenders = scored.iter().filter(|f| f.crap > cli.threshold).count();
    let load: f64 = scored
        .iter()
        .filter(|f| f.crap > cli.threshold)
        .map(|f| score::crap_load(f.cc, f.cov, cli.threshold))
        .sum();
    let summary = summarize(&scored, scanned, total_functions, cli.threshold, unparsed);

    scored.sort_by(|a, b| {
        b.crap
            .partial_cmp(&a.crap)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.cc.cmp(&a.cc))
            .then(a.file.cmp(&b.file))
            .then(a.line.cmp(&b.line))
    });

    let (candidates, below_threshold) = if cli.all {
        (scored.as_slice(), 0)
    } else {
        let split = scored.partition_point(|f| f.crap > cli.threshold);
        (&scored[..split], scored.len() - split)
    };

    let report = Report {
        ok: offenders == 0,
        coverage: coverage.as_ref().map(|c| CovInfo {
            source: c.source.clone(),
            format: c.format,
            files_matched: covered_files,
        }),
        diff: changed.as_ref().map(|c| DiffInfo {
            source: diff_source(cli),
            files: c.files,
            matched_files: diff_files,
            functions: diff_functions,
        }),
        summary: Summary {
            crap_load: load.ceil() as u32,
            ..summary
        },
        findings: candidates,
        dropped: Dropped {
            below_threshold,
            not_shown: 0,
            unchanged,
        },
    };

    if !cli.quiet {
        match cli.format {
            Format::Json => println!("{}", report.render_json(cli.max_tokens, cli.top)),
            Format::Summary => println!("{}", report.render_json(0, 0)),
            Format::Text => print!("{}", report.render_text()),
        }
    }

    if cli.fail_above && offenders > 0 {
        return Ok(ExitCode::from(1));
    }
    Ok(ExitCode::SUCCESS)
}

struct File {
    path: PathBuf,
    display: String,
    lang: Lang,
}

struct Parsed {
    path: String,
    funcs: Vec<score::Func>,
    /// tree-sitter hit ERROR or MISSING nodes; findings here are partial.
    unparsed: bool,
}

fn parse_file(parser: &mut TreeSitter, file: &File, cli: &Cli) -> Parsed {
    let empty = |path: &str| Parsed {
        path: path.to_string(),
        funcs: Vec::new(),
        unparsed: false,
    };
    let Ok(source) = std::fs::read(&file.path) else {
        return empty(&file.display);
    };
    if parser.set_language(&file.lang.grammar()).is_err() {
        return empty(&file.display);
    }
    let Some(tree) = parser.parse(&source, None) else {
        return empty(&file.display);
    };
    let options = Options {
        include_tests: cli.include_tests,
        snippet: cli.snippet,
    };
    let funcs = score::discover(&tree, &source, file.lang, options);
    Parsed {
        path: file.display.clone(),
        funcs,
        unparsed: tree.root_node().has_error(),
    }
}

fn to_finding(file: &Parsed, func: &score::Func, cov: Option<f64>, threshold: f64) -> Finding {
    let crap = score::crap(func.cc, cov);
    Finding {
        file: file.path.clone(),
        line: func.line,
        name: func.name.clone(),
        scope: func.scope.clone(),
        sig: func.sig.clone(),
        cc: func.cc,
        cov: cov.map(round1),
        crap: round1(crap),
        sev: score::severity(crap, threshold),
    }
}

/// `scored` is the set the report describes — every function normally, only the
/// changed ones in diff mode. `functions` stays the size of the whole scan so
/// the summary never quietly changes meaning between modes.
fn summarize(
    scored: &[Finding],
    files: usize,
    functions: usize,
    threshold: f64,
    unparsed: usize,
) -> Summary {
    let offenders = scored.iter().filter(|f| f.crap > threshold).count();
    let worst = scored.iter().map(|f| f.crap).fold(0.0, f64::max);
    let mean = if scored.is_empty() {
        0.0
    } else {
        scored.iter().map(|f| f.crap).sum::<f64>() / scored.len() as f64
    };
    Summary {
        files,
        functions,
        offenders,
        worst: round1(worst),
        mean: round1(mean),
        crap_load: 0,
        threshold,
        unparsed,
    }
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// The patch to score against, from git or from a file. `None` means score
/// everything.
fn load_diff(cli: &Cli) -> Result<Option<Changed>, String> {
    if let Some(path) = &cli.diff_file {
        let text = if path.as_os_str() == "-" {
            let mut buffer = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer)
                .map_err(|e| format!("cannot read diff from stdin: {e}"))?;
            buffer
        } else {
            std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read diff {}: {e}", path.display()))?
        };
        return Ok(Some(Changed::parse(&text)));
    }
    match &cli.diff {
        Some(rev) => Changed::from_git(rev, &git_dir(cli)).map(Some),
        None => Ok(None),
    }
}

/// Run git from the tree being scanned rather than from wherever the process
/// happens to be, so `crap /some/other/repo --diff main` diffs the right repo.
fn git_dir(cli: &Cli) -> PathBuf {
    let first = &cli.paths[0];
    let dir = if first.is_dir() {
        first.clone()
    } else {
        first.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    if dir.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        dir
    }
}

fn diff_source(cli: &Cli) -> String {
    match (&cli.diff, &cli.diff_file) {
        (Some(rev), _) => rev.clone(),
        (_, Some(path)) => path.display().to_string(),
        _ => String::new(),
    }
}

fn load_coverage(cli: &Cli) -> Result<Option<Coverage>, String> {
    if cli.no_coverage {
        return Ok(None);
    }
    let paths: Vec<PathBuf> = if cli.coverage.is_empty() {
        coverage::auto_discover().into_iter().collect()
    } else {
        cli.coverage.clone()
    };
    if paths.is_empty() {
        Ok(None)
    } else {
        Coverage::load_all(&paths).map(Some)
    }
}

fn collect(cli: &Cli) -> Result<Vec<File>, String> {
    // A typo'd path must not look like a clean report.
    for path in &cli.paths {
        if !path.exists() {
            return Err(format!("no such path: {}", path.display()));
        }
    }

    let empty: &[&str] = &[];
    let patterns: Vec<&str> = ALWAYS_SKIP
        .iter()
        .copied()
        .chain(
            if cli.include_tests { empty } else { TEST_SKIP }
                .iter()
                .copied(),
        )
        .chain(cli.exclude.iter().map(String::as_str))
        .collect();

    // Overrides are whitelists unless prefixed with `!`, and a single
    // whitelist turns the whole set into an allow-list, so every pattern here
    // is negated into an ignore rule.
    let mut overrides = OverrideBuilder::new(".");
    for pattern in &patterns {
        overrides
            .add(&format!("!{pattern}"))
            .map_err(|e| format!("bad exclude pattern {pattern}: {e}"))?;
    }
    let overrides = overrides
        .build()
        .map_err(|e| format!("bad exclude patterns: {e}"))?;

    let mut builder = WalkBuilder::new(&cli.paths[0]);
    for path in &cli.paths[1..] {
        builder.add(path);
    }
    builder
        .hidden(true)
        .parents(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .overrides(overrides);

    let mut files = Vec::new();
    for entry in builder.build() {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        let Some(lang) = Lang::from_path(path) else {
            continue;
        };
        files.push(File {
            path: path.to_path_buf(),
            display: display_path(path),
            lang,
        });
    }
    Ok(files)
}

fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    text.strip_prefix("./").unwrap_or(&text).to_string()
}
