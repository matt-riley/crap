# crap

Rank functions by **Change Risk Anti-Patterns** score — the code that is both
complicated *and* untested, and therefore the code most likely to break when
someone touches it.

```console
$ crap src/
  182.0  cc  13  cov     ?  src/main.rs:126  run
  156.0  cc  12  cov     ?  src/score.rs:57  discover
   90.0  cc   9  cov     ?  src/lang.rs:103  from_path (impl Lang)
```

TypeScript, JavaScript, Rust, Go and Lua, parsed with tree-sitter. Output is
JSON by default and **bounded by a token budget**, because the thing reading it
is usually a model with better things to spend context on.

## The metric

From Alberto Savoia and Bob Evans, popularised by Google's testing blog post
[This Code is CRAP](https://testing.googleblog.com/2011/02/this-code-is-crap.html):

```
CRAP(m) = comp(m)² × (1 − cov(m)/100)³ + comp(m)
```

`comp` is McCabe cyclomatic complexity — `1 + the number of branch sites` in
the function. `cov` is the percentage of the function's instrumented lines that
tests executed. The consequence worth internalising: **coverage matters
cubically, complexity quadratically.** A function of complexity 6 with no tests
scores 42. Cover it completely and it scores 6.

| coverage | complexity 5 | complexity 10 | complexity 20 |
|---|---|---|---|
| 0% | 30 | 110 | 420 |
| 50% | 8.1 | 22.5 | 70 |
| 100% | 5 | 10 | 20 |

crap4j's original guidance: **30 is the crappy line.** Below it you need
nothing; at complexity 26–30 you need 100% coverage to stay under it; above 31
no amount of testing saves you and the answer is to refactor. `crap` reports
`warn` above the threshold and `crit` above twice it (30/60 by default, and the
bands scale if you change `--threshold`).

The summary also reports **crap load**: the minimum work to drag every offender
under the threshold, counting one test per uncovered point plus one
extract-method per threshold's worth of complexity.

## Install

```bash
cargo install --path .
```

## Use

```bash
crap src/                                  # worst offenders, JSON
crap . --coverage lcov.info                # with real coverage
crap src/ --diff                           # only what I have not committed yet
crap src/ --diff 'main...HEAD'             # only what this branch added
crap src/ --format text                    # for humans
crap src/ --fail-above                     # CI gate, exit 1 if anything is over
crap src/ --max-tokens 4000 --top 50       # when you want more
```

Coverage is auto-discovered when the flag is omitted, from `lcov.info`,
`coverage/lcov.info`, `target/llvm-cov/lcov.info`, `coverage-final.json`,
`coverage/coverage-final.json`, `cobertura.xml`, `coverage.xml` and
`coverage.out`.

### Generating a coverage report

| Language | Command |
|---|---|
| Rust | `cargo llvm-cov --lcov --output-path lcov.info` |
| TS/JS | `vitest run --coverage --coverage.reporter=lcov` (or jest/nyc → `coverage/lcov.info`) |
| Go | `go test -coverprofile=coverage.out ./...` |
| Lua | any tool emitting LCOV or Cobertura |

## What counts as a branch

Complexity is `1 + ` one point per site:

| Language | Counted |
|---|---|
| TS / JS | `if`, `for`, `for…in/of`, `while`, `do`, `case`, `catch`, `? :`, `&&`, `\|\|`, `??` |
| Rust | `if`, `while`, `for`, `loop`, each `match` arm, `&&`, `\|\|`, `?` |
| Go | `if`, `for`, each `case` and `select` clause, `&&`, `\|\|` |
| Lua | `if`, `elseif`, `while`, `repeat`, `for`, `and`, `or` |

Two rules worth knowing:

- **Nested functions are scored separately.** A closure inside a function does
  not inflate its parent's complexity; both get their own row. Coverage then
  attributes to whichever function the lines actually belong to.
- **Every `match` arm and `case` clause counts**, including `_` and `default`,
  because each one is a path through the function that someone can break.
  Enum-dispatch `match`es therefore score higher than they look — that is the
  intended reading, and it is why `--all` exists.

## No coverage report means worst case

If no coverage file is found, every function is scored as 0% covered, so every
score is the `cc² + cc` upper bound. That is deliberate — a risk metric should
assume the worst until shown otherwise — but it is also the most common way to
misread this tool. The JSON says `"coverage": null` when this happens, and
individual functions get `"cov": null` when the report exists but says nothing
about them. `null` means *unmeasured*; `0.0` means *measured and untested*.

Coverage files name paths relative to wherever their tool ran, so `crap` also
matches on trailing path suffixes (`src/a/mod.rs` → `a/mod.rs` → `mod.rs`) and
refuses to guess when a suffix is ambiguous. `coverage.files_matched` in the
summary tells you how many scanned files actually matched — compare it against
`summary.files` before believing anything.

## Scoring only what you changed

During feature work the useful question is "is *my* code risky?", not "is
this repository risky?" — a 900-function codebase will always have offenders
somewhere. `--diff` narrows the report to the functions a branch touched.

```bash
crap src/ --diff                  # uncommitted work (against HEAD)
crap src/ --diff main             # uncommitted work plus the difference from main
crap src/ --diff 'main...HEAD'    # committed work since branching
crap src/ --diff main --fail-above   # fail the build only on my code
crap src/ --diff-file ci.patch    # from a patch, no git required (`-` for stdin)
```

The revspec is passed straight to `git diff`, so anything git understands works.
Git runs from the scanned tree rather than the process's working directory, so
`crap /some/other/repo --diff main` diffs the repo you pointed at.

**What counts as changed.** A function is scored when a line *added* by the diff
falls inside it. Deleted lines have no position in the new file, so removing
code from inside a function does not by itself flag it — the usual caveat of
diff-based tooling. Untracked files are included in full, because a new file is
the most likely thing to be written during feature work and `git diff` alone
does not show it.

**The summary keeps its meaning.** `summary.files` and `summary.functions`
always describe the whole scan; `offenders`, `worst`, `mean` and `crap_load`
describe the reported set, which in diff mode is the changed functions. A `diff`
block says what was diffed against and how much of it matched:

```json
"diff": {"source": "main...HEAD", "files": 4, "matched_files": 4, "functions": 12},
"dropped": {"below_threshold": 33, "not_shown": 0, "unchanged": 88}
```

`dropped.unchanged` is the count the diff did not touch, so a narrow report is
never mistaken for a clean one. If `matched_files` is 0, the paths in the diff
did not line up with anything scanned and the empty result means nothing.

## Output

JSON is the default and it is budgeted. The preamble (summary, coverage status)
always ships; findings are added worst-first until `--top` or the token budget
is reached; then a `dropped` block says what was withheld and why:

```json
{
  "v": 1,
  "ok": false,
  "coverage": {"source": "lcov.info", "format": "lcov", "files_matched": 118},
  "summary": {"files": 128, "functions": 1204, "offenders": 12, "worst": 156.0,
              "mean": 3.41, "crap_load": 47, "threshold": 30},
  "findings": [
    {"file": "src/foo.rs", "line": 24, "name": "bar", "scope": "impl Foo",
     "cc": 12, "cov": 0.0, "crap": 156.0, "sev": "crit"}
  ],
  "dropped": {"below_threshold": 1147, "not_shown": 4}
}
```

`below_threshold` means "re-run with `--all`"; `not_shown` means "raise
`--top` or `--max-tokens`". A `summary.unparsed` count appears only when
tree-sitter hit syntax errors, so a file that failed to parse can never be
mistaken for a file with nothing wrong in it. Tokens are estimated as `chars / 4`. The budget is
a ceiling, not a hint: at `--max-tokens 4000` a repo with 5,500 functions still
produces a document under 4,000 tokens. `--max-tokens 0` gives the preamble
alone, which is also what `--format summary` does.

Paths stay relative, so the output is portable and short. `--snippet` adds the
declaration's first line when you need to see the shape of the thing without
opening the file.

## Exit codes

`0` report produced, `1` threshold tripped and `--fail-above` was set, `2`
error (unreadable coverage file, a bad exclude pattern, a path that does not
exist, a git command that failed). `"ok"` in the JSON carries the
same verdict, so a model never has to infer it from a status code.

## Skips

Always skipped: `node_modules/`, `target/`, `dist/`, `build/`, `out/`,
`vendor/`, `coverage/`, `__pycache__/`, plus anything git-ignored.

Skipped unless `--include-tests`: `tests/`, `test/`, `__tests__/`, `spec/`,
`testdata/`, `*.test.*`, `*.spec.*`, `*_test.go`, and in Rust `#[cfg(test)]`
modules and `#[test]` functions. Test code is complex by nature and is not what
anyone means by risky untested logic. Note that these are path patterns matched
against the path as printed — `crap tests/` finds nothing without
`--include-tests`.

`--exclude` takes further gitignore-style patterns.

## Limitations

- No diff or changed-lines mode: it scores what you point it at, not what a
  pull request touched.
- No baseline or regression comparison — `--fail-above` is the whole CI story.
  `--diff` narrows the gate to a branch's own changes, which covers most of it.
- JavaScript is parsed with the TypeScript/TSX grammars rather than a dedicated
  JS grammar. Fine in practice; add `tree-sitter-javascript` if it ever isn't.
- Coverage is line-based, so a function with a single untested line inside a
  covered block scores exactly that line as uncovered.

## Development

```bash
.pi/verify        # fmt + clippy + tests
```

The fixtures in `tests/fixtures/` annotate each function with the complexity a
human counts by hand; a wrong node kind in a language spec fails there.
