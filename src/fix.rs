//! Repairs that can be proved from what is already in the project.
//!
//! Nothing here guesses. A repair is only produced when there is exactly one
//! possible answer: the file exists under a different spelling, the uid already
//! resolves somewhere, or exactly one file in the project carries that name.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::project::Project;

#[derive(Debug, Clone, PartialEq)]
pub struct Fix {
    pub check: &'static str,
    /// res:// path of the file to edit.
    pub file: String,
    pub line: usize,
    pub old: String,
    pub new: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct Applied {
    pub fix: Fix,
    pub ok: bool,
    pub note: String,
}

/// Files carrying each base name, used to recognise a file that only moved.
pub fn by_name(p: &Project) -> BTreeMap<String, Vec<String>> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in &p.files {
        if let Some(name) = f.rsplit('/').next() {
            m.entry(name.to_string()).or_default().push(f.clone());
        }
    }
    m
}

/// The single file that could be meant by a missing path, if there is one.
pub fn moved_target(p: &Project, missing: &str) -> Option<String> {
    let name = missing.rsplit('/').next()?;
    if name.is_empty() {
        return None;
    }
    let candidates = by_name(p);
    let list = candidates.get(name)?;
    match list.len() {
        1 => Some(list[0].clone()),
        _ => None,
    }
}

/// A repair is only offered when the text in the file is the path itself, so
/// the replacement cannot disturb a locale suffix, a `..` step or a `*` prefix.
pub fn rewritable(raw: &str, path: &str) -> bool {
    raw == path
}

pub fn apply(root: &Path, fixes: &[Fix]) -> Vec<Applied> {
    let mut by_file: BTreeMap<String, Vec<Fix>> = BTreeMap::new();
    for f in fixes {
        by_file.entry(f.file.clone()).or_default().push(f.clone());
    }
    let mut out = Vec::new();
    for (res, list) in by_file {
        let rel = res.strip_prefix("res://").unwrap_or(&res);
        let path = root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                for fix in list {
                    out.push(Applied { fix, ok: false, note: format!("cannot read: {}", e) });
                }
                continue;
            }
        };
        let text = match String::from_utf8(bytes) {
            Ok(t) => t,
            Err(_) => {
                for fix in list {
                    out.push(Applied { fix, ok: false, note: "not UTF-8".into() });
                }
                continue;
            }
        };
        let mut lines: Vec<String> = text.split_inclusive('\n').map(|s| s.to_string()).collect();
        let mut touched = false;
        let mut results = Vec::new();
        let mut list = list;
        list.sort_by_key(|f| f.line);
        // A value in a Godot config file may run over several physical lines, so
        // the recorded line is where the entry starts, not necessarily where the
        // path is written. The search moves forward only, which keeps repeated
        // paths in one entry in their own places.
        for fix in list {
            if fix.new.contains(&fix.old) {
                results.push(Applied {
                    fix,
                    ok: false,
                    note: "the replacement would still contain the old text".into(),
                });
                continue;
            }
            let start = fix.line.saturating_sub(1);
            // Replaced text no longer matches, so searching forward from the
            // entry's first line finds each occurrence exactly once.
            let hit = (start..lines.len()).find(|i| lines[*i].contains(&fix.old));
            match hit {
                Some(i) => {
                    lines[i] = lines[i].replacen(&fix.old, &fix.new, 1);
                    touched = true;
                    results.push(Applied { fix, ok: true, note: String::new() });
                }
                None => results.push(Applied {
                    fix,
                    ok: false,
                    note: "that text is no longer in the file".into(),
                }),
            }
        }
        if touched {
            if let Err(e) = fs::write(&path, lines.concat()) {
                for r in results.iter_mut() {
                    r.ok = false;
                    r.note = format!("cannot write: {}", e);
                }
            }
        }
        out.extend(results);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &str) {
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, body).unwrap();
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("refcheck-fix-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn only_the_named_text_on_the_line_changes() {
        let d = scratch("one");
        write(
            &d,
            "a.tscn",
            "line one\n[ext_resource path=\"res://old.png\" id=\"1\"]\nline three\n",
        );
        let fixes = vec![Fix {
            check: "missing-resource",
            file: "res://a.tscn".into(),
            line: 2,
            old: "res://old.png".into(),
            new: "res://art/old.png".into(),
            reason: String::new(),
        }];
        let done = apply(&d, &fixes);
        assert!(done[0].ok, "{:?}", done[0]);
        let after = fs::read_to_string(d.join("a.tscn")).unwrap();
        assert_eq!(
            after,
            "line one\n[ext_resource path=\"res://art/old.png\" id=\"1\"]\nline three\n"
        );
        let _ = fs::remove_dir_all(&d);
    }

    /// A Godot config value can run over several physical lines. The recorded
    /// line is where the entry begins, so the search has to look forward.
    #[test]
    fn a_value_spanning_lines_is_still_repaired() {
        let d = scratch("multi");
        write(
            &d,
            "project.godot",
            "_global_script_classes=[{\n\"path\": \"res://src/a.gd\"\n}, {\n\"path\": \"res://src/b.gd\"\n}]\n",
        );
        let fixes = vec![
            Fix {
                check: "missing-resource",
                file: "res://project.godot".into(),
                line: 1,
                old: "res://src/a.gd".into(),
                new: "res://code/a.gd".into(),
                reason: String::new(),
            },
            Fix {
                check: "missing-resource",
                file: "res://project.godot".into(),
                line: 1,
                old: "res://src/b.gd".into(),
                new: "res://code/b.gd".into(),
                reason: String::new(),
            },
        ];
        let done = apply(&d, &fixes);
        assert!(done.iter().all(|a| a.ok), "{:?}", done);
        let after = fs::read_to_string(d.join("project.godot")).unwrap();
        assert!(after.contains("res://code/a.gd"));
        assert!(after.contains("res://code/b.gd"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn windows_line_endings_survive_a_repair() {
        let d = scratch("crlf");
        write(&d, "a.tscn", "one\r\npath=\"res://old.png\"\r\nthree\r\n");
        let fixes = vec![Fix {
            check: "case-mismatch",
            file: "res://a.tscn".into(),
            line: 2,
            old: "res://old.png".into(),
            new: "res://Old.png".into(),
            reason: String::new(),
        }];
        assert!(apply(&d, &fixes)[0].ok);
        let after = fs::read_to_string(d.join("a.tscn")).unwrap();
        assert_eq!(after, "one\r\npath=\"res://Old.png\"\r\nthree\r\n");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_repair_whose_text_is_gone_is_reported_not_forced() {
        let d = scratch("gone");
        write(&d, "a.tscn", "nothing to see\n");
        let fixes = vec![Fix {
            check: "case-mismatch",
            file: "res://a.tscn".into(),
            line: 1,
            old: "res://old.png".into(),
            new: "res://new.png".into(),
            reason: String::new(),
        }];
        let done = apply(&d, &fixes);
        assert!(!done[0].ok);
        assert_eq!(fs::read_to_string(d.join("a.tscn")).unwrap(), "nothing to see\n");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_path_written_with_a_suffix_is_left_alone() {
        // `res://voice.wav:es` in a translation remap is not the path itself.
        assert!(!rewritable("res://voice.wav:es", "res://voice.wav"));
        assert!(rewritable("res://voice.wav", "res://voice.wav"));
    }
}
