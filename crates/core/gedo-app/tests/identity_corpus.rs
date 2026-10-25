//! The reference identity corpus (FEAT-014, User Story 6, research R9).
//!
//! Scripted edits of real Rust files, each with its known outcome: which old entity every new
//! entity continues, or none. The test reports identity accuracy and false continuations per
//! category and fails below the targets of SC-001 and SC-002.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use gedo_ast::{ChangeSet, EntityTokens, NewFile, OldFile, match_change};
use gedo_core::identity::{AstNodeId, BlobAstMap};
use gedo_core::ids::GitOid;
use gedo_core::ports::Parser;
use gedo_core::syntax::{CodeEntity, EntityKind, SyntaxMap};
use gedo_parse::TreeSitterParser;

/// SC-001: identities that should be preserved and are, overall.
const MIN_ACCURACY_OVERALL: f64 = 0.95;
/// SC-001: the same, per category.
const MIN_ACCURACY_PER_CATEGORY: f64 = 0.90;
/// SC-002: new entities that wrongly keep an old identity.
const MAX_FALSE_CONTINUATIONS: f64 = 0.01;

/// Cases drawn per file and category.
const CASES_PER_FILE: usize = 6;
const SEED: u64 = 0x6765_646f_2d69_6431;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../adapters/gedo-parse/tests/fixtures/rust")
}

/// The real files of the corpus: `(repository path, text)`.
fn sources() -> Vec<(String, String)> {
    let mut files = Vec::new();
    for (directory, prefix) in [("formatted/default/src", "src"), ("src", "src/conformance")] {
        let mut paths: Vec<_> = std::fs::read_dir(fixtures().join(directory))
            .expect("fixtures are readable")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .collect();
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path).expect("fixture is UTF-8");
            let name = path
                .file_name()
                .expect("file name")
                .to_string_lossy()
                .into_owned();
            if name == "lib.rs" || text.contains('\r') || text.starts_with('\u{feff}') {
                continue;
            }
            files.push((format!("{prefix}/{name}"), text));
        }
    }
    files
}

// --- Matching harness ---------------------------------------------------------------------------

fn syntax(path: &str, text: &str) -> SyntaxMap {
    let parsed = TreeSitterParser::new()
        .parse(path, text.as_bytes())
        .expect("parses");
    gedo_ast::normalize(&parsed, text.as_bytes()).expect("normalizes")
}

fn blob(text: &str) -> GitOid {
    GitOid::from_bytes(&blake3::hash(text.as_bytes()).as_bytes()[..20]).expect("20 bytes")
}

/// A version of a few files: each file's map and syntax map.
type Version = BTreeMap<String, (BlobAstMap, SyntaxMap)>;

fn index(files: &BTreeMap<String, String>) -> Version {
    step(&Version::new(), files)
}

/// Matches `files` (the whole new version) against `previous`.
fn step(previous: &Version, files: &BTreeMap<String, String>) -> Version {
    let mut change = ChangeSet::default();
    for (map, syntax_map) in previous.values() {
        let tokens = map
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                (!node.is_detached())
                    .then(|| syntax_map.entities.get(index).map(EntityTokens::of))
                    .flatten()
            })
            .collect();
        change.old.push(OldFile {
            map: map.clone(),
            tokens,
        });
    }
    let mut syntaxes = BTreeMap::new();
    for (path, text) in files {
        let syntax_map = syntax(path, text);
        change.new.push(NewFile {
            blob: blob(text),
            syntax: syntax_map.clone(),
        });
        syntaxes.insert(path.clone(), syntax_map);
    }
    match_change(&change, &BTreeSet::new())
        .maps
        .into_iter()
        .map(|map| {
            let syntax_map = syntaxes.remove(&map.path).expect("one map per file");
            (map.path.clone(), (map, syntax_map))
        })
        .collect()
}

// --- Edits ---------------------------------------------------------------------------------------

/// A location of an entity: file and qualified path.
type Key = (String, String);

/// One scripted edit: the new files and, for every old entity, where it should be afterwards
/// (`None`: it should be gone). New entities not in the image should be new.
struct Case {
    category: &'static str,
    after: BTreeMap<String, String>,
    expected: BTreeMap<Key, Option<Key>>,
    /// Every old entity is expected somewhere in the new maps, placed or carried (syntax errors).
    carried: bool,
}

struct Rng(u64);

impl Rng {
    fn below(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % bound.max(1)
    }
}

/// The functions and methods of a file that edits can target: unique names, simple bodies.
fn targets(syntax_map: &SyntaxMap, text: &str) -> Vec<usize> {
    syntax_map
        .entities
        .iter()
        .enumerate()
        .filter(|(_, entity)| matches!(entity.kind, EntityKind::Function | EntityKind::Method))
        .filter(|(_, entity)| text.matches(&format!("fn {}", entity.name)).count() == 1)
        .filter(|(_, entity)| body_open(entity_text(text, entity)).is_some())
        .map(|(index, _)| index)
        .collect()
}

fn entity_text<'t>(text: &'t str, entity: &CodeEntity) -> &'t str {
    &text[entity.byte_range.as_usize()]
}

/// Offset of the body's `{` in a function's text: the first `{` after `fn` outside parentheses,
/// brackets and angle brackets.
fn body_open(function: &str) -> Option<usize> {
    let start = function.find("fn ")?;
    let mut depth = 0i32;
    for (offset, character) in function[start..].char_indices() {
        match character {
            '(' | '[' | '<' => depth += 1,
            ')' | ']' => depth -= 1,
            '>' if !function[start..start + offset].ends_with('-')
                && !function[start..start + offset].ends_with('=') =>
            {
                depth -= 1
            }
            '{' if depth == 0 => return Some(start + offset),
            ';' if depth == 0 => return None,
            _ => {}
        }
    }
    None
}

/// The identity image: every entity stays where it is.
fn identity_image(before: &Version) -> BTreeMap<Key, Option<Key>> {
    before
        .iter()
        .flat_map(|(path, (map, _))| {
            map.nodes.iter().map(move |node| {
                let key = (path.clone(), node.qualified_path.clone());
                (key.clone(), Some(key))
            })
        })
        .collect()
}

/// Moves every key equal to or below `from` (in `path`) to `to` (in `new_path`).
fn remap(image: &mut BTreeMap<Key, Option<Key>>, path: &str, from: &str, new_path: &str, to: &str) {
    for (key, target) in image.iter_mut() {
        if key.0 != path {
            continue;
        }
        let rest = if key.1 == from {
            Some(String::new())
        } else {
            key.1
                .strip_prefix(&format!("{from}::"))
                .map(|rest| format!("::{rest}"))
        };
        if let Some(rest) = rest {
            *target = Some((new_path.to_owned(), format!("{to}{rest}")));
        }
    }
}

fn remove(image: &mut BTreeMap<Key, Option<Key>>, path: &str, qualified_path: &str) {
    for (key, target) in image.iter_mut() {
        if key.0 == path && (key.1 == qualified_path || key.1.starts_with(&format!("{qualified_path}::"))) {
            *target = None;
        }
    }
}

fn replace_range(text: &str, range: std::ops::Range<usize>, with: &str) -> String {
    format!("{}{}{}", &text[..range.start], with, &text[range.end..])
}

/// Statements inserted at the start of a body.
fn statements(count: usize, salt: usize) -> String {
    (0..count)
        .map(|index| format!("\n    let gedo_probe_{salt}_{index} = {index} + {salt};"))
        .collect()
}

fn body_lines(function: &str) -> usize {
    function.lines().count().saturating_sub(2).max(1)
}

fn module_of(path: &str) -> String {
    let stem = path
        .trim_end_matches(".rs")
        .trim_start_matches("src/")
        .replace('/', "::");
    format!("crate::{stem}")
}

/// Every case of the corpus.
fn cases(rng: &mut Rng) -> Vec<(BTreeMap<String, String>, Case)> {
    let mut all = Vec::new();
    for (path, text) in sources() {
        let before_files = BTreeMap::from([(path.clone(), text.clone())]);
        let before = index(&before_files);
        let syntax_map = &before[&path].1;
        let candidates = targets(syntax_map, &text);
        if candidates.is_empty() {
            continue;
        }
        for draw in 0..CASES_PER_FILE {
            let entity = &syntax_map.entities[candidates[rng.below(candidates.len())]];
            let function = entity_text(&text, entity);
            let range = entity.byte_range.as_usize();
            let open = range.start + body_open(function).expect("targets have bodies");
            let name = entity.name.clone();
            let qualified = entity.qualified_path.clone();
            let parent = qualified
                .strip_suffix(&format!("::{name}"))
                .unwrap_or(&qualified)
                .to_owned();
            let one = |category: &'static str, after: String, image: BTreeMap<Key, Option<Key>>| Case {
                category,
                after: BTreeMap::from([(path.clone(), after)]),
                expected: image,
                carried: false,
            };
            let mut cases = Vec::new();

            // Body edits: one statement, a quarter and a half of the body added.
            for (category, fraction) in [
                ("body: one statement", 0usize),
                ("body: a quarter", 4),
                ("body: a half", 2),
            ] {
                let count = if fraction == 0 {
                    1
                } else {
                    body_lines(function).div_ceil(fraction)
                };
                let after = replace_range(&text, open + 1..open + 1, &statements(count, draw));
                cases.push(one(category, after, identity_image(&before)));
            }

            // Signature: a parameter added; visibility changed.
            let close = function[..body_open(function).expect("body")]
                .rfind(')')
                .map(|offset| range.start + offset);
            if let Some(close) = close {
                let empty = text[..close].trim_end().ends_with('(');
                let parameter = if empty {
                    "gedo_extra: u8"
                } else {
                    ", gedo_extra: u8"
                };
                cases.push(one(
                    "signature: parameter added",
                    replace_range(&text, close..close, parameter),
                    identity_image(&before),
                ));
            }
            let fn_at = range.start + function.find("fn ").expect("fn");
            let visibility = if text[..fn_at].ends_with("pub ") {
                replace_range(&text, fn_at - 4..fn_at, "")
            } else {
                replace_range(&text, fn_at..fn_at, "pub(crate) ")
            };
            cases.push(one("signature: visibility", visibility, identity_image(&before)));

            // Renames, alone and with a body edit.
            let name_at = fn_at + 3;
            let renamed_name = format!("{name}_renamed");
            let renamed = replace_range(&text, name_at..name_at + name.len(), &renamed_name);
            let mut image = identity_image(&before);
            remap(
                &mut image,
                &path,
                &qualified,
                &path,
                &format!("{parent}::{renamed_name}"),
            );
            cases.push(one("rename", renamed.clone(), image.clone()));
            let renamed_open = open + renamed_name.len() - name.len();
            let edited = replace_range(
                &renamed,
                renamed_open + 1..renamed_open + 1,
                &statements(body_lines(function).div_ceil(4), draw),
            );
            cases.push(one("rename and body edit", edited, image));

            // Moves: only free functions can move out of their container.
            if entity.container.is_none() && entity.kind == EntityKind::Function {
                let without = replace_range(&text, range.clone(), "");
                let module = module_of(&path);
                let mut image = identity_image(&before);
                remap(
                    &mut image,
                    &path,
                    &qualified,
                    &path,
                    &format!("{module}::gedo_moved::{name}"),
                );
                cases.push(one(
                    "move: another module",
                    format!("{without}\npub mod gedo_moved {{\n{function}\n}}\n"),
                    image,
                ));

                let target = path.replace(".rs", "_moved.rs");
                let mut image = identity_image(&before);
                remap(
                    &mut image,
                    &path,
                    &qualified,
                    &target,
                    &format!("{}::{name}", module_of(&target)),
                );
                cases.push(Case {
                    category: "move: another file",
                    after: BTreeMap::from([
                        (path.clone(), without.clone()),
                        (target, format!("{function}\n")),
                    ]),
                    expected: image,
                    carried: false,
                });

                // Copy: the original stays, the copy is new.
                let copy = function.replacen(&format!("fn {name}"), &format!("fn {name}_copy"), 1);
                cases.push(one("copy", format!("{text}\n{copy}\n"), identity_image(&before)));

                // Split: the original delegates to a new function holding its old body.
                let body = &function[body_open(function).expect("body")..];
                let rest = format!("\nfn {name}_rest() {body}\n");
                let split = replace_range(&text, open..range.end, &format!("{{\n    {name}_rest()\n}}"));
                cases.push(one("split", format!("{split}{rest}"), identity_image(&before)));

                // Deletion.
                let mut image = identity_image(&before);
                remove(&mut image, &path, &qualified);
                cases.push(one("deletion", without.clone(), image.clone()));

                // Replacement: an unrelated function written where the deleted one was.
                let replacement = format!(
                    "pub fn gedo_replacement_{draw}(left: &[u8], right: &[u8]) -> bool {{\n    left.len() == right.len() && left.iter().zip(right).all(|(a, b)| a == b)\n}}"
                );
                cases.push(one(
                    "replacement",
                    replace_range(&text, range.clone(), &replacement),
                    image,
                ));

                // Merge: another free function's body folded into this one, that function removed.
                let others: Vec<&CodeEntity> = candidates
                    .iter()
                    .map(|&index| &syntax_map.entities[index])
                    .filter(|other| {
                        other.container.is_none() && other.kind == EntityKind::Function && other.name != name
                    })
                    .collect();
                if !others.is_empty() {
                    let other = others[rng.below(others.len())];
                    let other_text = entity_text(&text, other);
                    let other_range = other.byte_range.as_usize();
                    let folded = format!("\n    {};", &other_text[body_open(other_text).expect("body")..]);
                    let merged = if other_range.start > open {
                        replace_range(
                            &replace_range(&text, other_range, ""),
                            open + 1..open + 1,
                            &folded,
                        )
                    } else {
                        let shifted = open - other_range.len();
                        replace_range(
                            &replace_range(&text, other_range, ""),
                            shifted + 1..shifted + 1,
                            &folded,
                        )
                    };
                    let mut image = identity_image(&before);
                    remove(&mut image, &path, &other.qualified_path);
                    cases.push(one("merge", merged, image));
                }
            }

            // File renamed.
            let renamed_path = path.replace(".rs", "_renamed.rs");
            let mut image = identity_image(&before);
            for target in image.values_mut() {
                if let Some((_, qualified_path)) = target.take() {
                    let module = module_of(&path);
                    let suffix = qualified_path
                        .strip_prefix(&module)
                        .unwrap_or(&qualified_path)
                        .to_owned();
                    *target = Some((
                        renamed_path.clone(),
                        format!("{}{suffix}", module_of(&renamed_path)),
                    ));
                }
            }
            cases.push(Case {
                category: "move: file renamed",
                after: BTreeMap::from([(renamed_path.clone(), text.clone())]),
                expected: image,
                carried: false,
            });

            // Addition of an unrelated function.
            let addition = format!(
                "{text}\npub fn gedo_added_{draw}(input: &str) -> usize {{\n    input.split_whitespace().count()\n}}\n"
            );
            cases.push(one("addition", addition, identity_image(&before)));

            // A syntax error: the body's closing brace removed.
            let last_brace = range.start + function.rfind('}').expect("closing brace");
            let mut damaged = one(
                "syntax error",
                replace_range(&text, last_brace..last_brace + 1, ""),
                identity_image(&before),
            );
            damaged.carried = true;
            cases.push(damaged);

            all.extend(cases.into_iter().map(|case| (before_files.clone(), case)));
        }

        // Reorder: two top-level functions swapped (once per file).
        let top: Vec<&CodeEntity> = candidates
            .iter()
            .map(|&index| &syntax_map.entities[index])
            .filter(|entity| entity.container.is_none())
            .collect();
        if let [first, .., last] = top.as_slice() {
            let (first_text, last_text) = (entity_text(&text, first), entity_text(&text, last));
            let swapped = replace_range(
                &replace_range(&text, last.byte_range.as_usize(), first_text),
                first.byte_range.as_usize(),
                last_text,
            );
            all.push((
                before_files.clone(),
                Case {
                    category: "reorder",
                    after: BTreeMap::from([(path.clone(), swapped)]),
                    expected: identity_image(&before),
                    carried: false,
                },
            ));
        }
    }

    // Formatter runs: the committed rustfmt variants of feature 002.
    let default = fixtures().join("formatted/default/src");
    for variant in ["narrow", "wide", "tabs"] {
        let mut names: Vec<_> = std::fs::read_dir(&default)
            .expect("readable")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        names.sort();
        for name in names {
            let name = name.to_string_lossy().into_owned();
            let (Ok(before_text), Ok(after_text)) = (
                std::fs::read_to_string(default.join(&name)),
                std::fs::read_to_string(fixtures().join(format!("formatted/{variant}/src")).join(&name)),
            ) else {
                continue;
            };
            let path = format!("src/{name}");
            let before_files = BTreeMap::from([(path.clone(), before_text)]);
            let before = index(&before_files);
            all.push((
                before_files,
                Case {
                    category: "formatter run",
                    after: BTreeMap::from([(path, after_text)]),
                    expected: identity_image(&before),
                    carried: false,
                },
            ));
        }
    }
    all
}

// --- Measurement ----------------------------------------------------------------------------------

#[derive(Default)]
struct Tally {
    cases: usize,
    should_keep: usize,
    kept: usize,
    should_be_new: usize,
    wrongly_kept: usize,
}

fn measure(before_files: &BTreeMap<String, String>, case: &Case, tally: &mut Tally) {
    let before = index(before_files);
    let after = step(&before, &case.after);
    let old_ids: BTreeMap<Key, AstNodeId> = before
        .iter()
        .flat_map(|(path, (map, _))| {
            map.nodes
                .iter()
                .map(move |node| ((path.clone(), node.qualified_path.clone()), node.id))
        })
        .collect();
    let mut new_ids: BTreeMap<Key, AstNodeId> = BTreeMap::new();
    let mut all_new: BTreeSet<AstNodeId> = BTreeSet::new();
    for (path, (map, _)) in &after {
        for node in &map.nodes {
            all_new.insert(node.id);
            if !node.is_detached() {
                new_ids
                    .entry((path.clone(), node.qualified_path.clone()))
                    .or_insert(node.id);
            }
        }
    }
    let mut expected_targets = BTreeSet::new();
    for (key, target) in &case.expected {
        let Some(target) = target else {
            continue;
        };
        expected_targets.insert(target.clone());
        tally.should_keep += 1;
        let id = old_ids[key];
        let preserved = if case.carried {
            all_new.contains(&id)
        } else {
            new_ids.get(target) == Some(&id)
        };
        if preserved {
            tally.kept += 1;
        }
    }
    // After a syntax error the parser's recovery re-nests what follows the damage, so carried
    // entities appear under keys the image cannot predict: such cases measure preservation only.
    let old: BTreeSet<AstNodeId> = old_ids.values().copied().collect();
    for (key, id) in new_ids.iter().filter(|_| !case.carried) {
        if !expected_targets.contains(key) {
            tally.should_be_new += 1;
            if old.contains(id) {
                tally.wrongly_kept += 1;
            }
        }
    }
    tally.cases += 1;
}

#[test]
fn identity_accuracy_meets_the_targets() {
    let mut rng = Rng(SEED);
    let mut tallies: BTreeMap<&'static str, Tally> = BTreeMap::new();
    for (before, case) in cases(&mut rng) {
        measure(&before, &case, tallies.entry(case.category).or_default());
    }
    let mut report = String::from("category                      cases   accuracy   false continuations\n");
    let mut total = Tally::default();
    let mut failures = Vec::new();
    for (category, tally) in &tallies {
        let accuracy = ratio(tally.kept, tally.should_keep);
        let _ = writeln!(report, "{category:<28} {}", row(tally));
        if accuracy < MIN_ACCURACY_PER_CATEGORY {
            failures.push(format!("{category}: accuracy {:.2}%", accuracy * 100.0));
        }
        total.cases += tally.cases;
        total.should_keep += tally.should_keep;
        total.kept += tally.kept;
        total.should_be_new += tally.should_be_new;
        total.wrongly_kept += tally.wrongly_kept;
    }
    let accuracy = ratio(total.kept, total.should_keep);
    let false_rate = ratio(total.wrongly_kept, total.should_be_new);
    let _ = writeln!(report, "{:<28} {}", "overall", row(&total));
    println!("{report}");
    if accuracy < MIN_ACCURACY_OVERALL {
        failures.push(format!("overall accuracy {:.2}%", accuracy * 100.0));
    }
    if false_rate > MAX_FALSE_CONTINUATIONS {
        failures.push(format!("false continuations {:.2}%", false_rate * 100.0));
    }
    assert!(total.cases > 500, "the corpus has only {} cases", total.cases);
    assert!(failures.is_empty(), "{}\n{report}", failures.join("\n"));
}

fn row(tally: &Tally) -> String {
    let false_rate = if tally.should_be_new == 0 {
        "n/a".to_owned()
    } else {
        format!(
            "{:.2}% ({}/{})",
            ratio(tally.wrongly_kept, tally.should_be_new) * 100.0,
            tally.wrongly_kept,
            tally.should_be_new
        )
    };
    format!(
        "{:>6}   {:>7.2}%   {false_rate}",
        tally.cases,
        ratio(tally.kept, tally.should_keep) * 100.0
    )
}

fn ratio(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        1.0
    } else {
        part as f64 / whole as f64
    }
}
