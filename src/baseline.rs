//! A list of findings a project has agreed to live with for now, so the gate
//! can be switched on today and only new problems fail the build.
//!
//! The format is one finding per line, tab separated, so it diffs and merges
//! like the rest of a repository. The line number is deliberately not part of
//! it: editing the lines above a finding must not wake it up again.

use std::collections::BTreeSet;

use crate::finding::Finding;

pub fn fingerprint(f: &Finding) -> String {
    format!("{}\t{}\t{}\t{}", f.check, f.project, f.file, f.message)
}

pub fn render(findings: &[Finding], today: &str) -> String {
    let mut lines: BTreeSet<String> = BTreeSet::new();
    for f in findings {
        lines.insert(fingerprint(f));
    }
    let mut out = String::new();
    out.push_str(&format!(
        "# godot-refcheck baseline, {} findings, written {}\n\
         # check<TAB>project<TAB>file<TAB>message. Delete a line to start failing on it again.\n",
        lines.len(),
        today
    ));
    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
    out
}

pub fn parse(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

/// Splits findings into the ones the baseline already covers and the rest.
pub fn split(findings: Vec<Finding>, known: &BTreeSet<String>) -> (Vec<Finding>, usize) {
    let mut kept = Vec::new();
    let mut silenced = 0usize;
    for f in findings {
        if known.contains(&fingerprint(&f)) {
            silenced += 1;
        } else {
            kept.push(f);
        }
    }
    (kept, silenced)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finding::Level;

    fn f(check: &'static str, file: &str, message: &str, line: usize) -> Finding {
        Finding {
            project: String::new(),
            check,
            level: Level::Error,
            file: file.into(),
            line,
            message: message.into(),
            evidence: String::new(),
        }
    }

    #[test]
    fn a_finding_is_silenced_by_its_own_line() {
        let items =
            vec![f("missing-resource", "res://a.tscn", "res://b.png is not in the project", 4)];
        let text = render(&items, "2026-09-22");
        let known = parse(&text);
        let (kept, silenced) = split(items, &known);
        assert!(kept.is_empty());
        assert_eq!(silenced, 1);
    }

    /// Adding a line above a finding must not wake it up again, so the line
    /// number is deliberately not part of the fingerprint.
    #[test]
    fn moving_a_finding_down_the_file_keeps_it_silenced() {
        let before =
            vec![f("missing-resource", "res://a.tscn", "res://b.png is not in the project", 4)];
        let known = parse(&render(&before, "2026-09-22"));
        let after =
            vec![f("missing-resource", "res://a.tscn", "res://b.png is not in the project", 40)];
        let (kept, silenced) = split(after, &known);
        assert!(kept.is_empty());
        assert_eq!(silenced, 1);
    }

    #[test]
    fn a_different_finding_still_gets_through() {
        let known = parse(&render(
            &[f("missing-resource", "res://a.tscn", "res://b.png is not in the project", 4)],
            "2026-09-22",
        ));
        let (kept, silenced) = split(
            vec![f("missing-resource", "res://a.tscn", "res://c.png is not in the project", 4)],
            &known,
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(silenced, 0);
    }

    #[test]
    fn comments_and_blank_lines_are_not_fingerprints() {
        let known = parse("# a comment\n\n  \nmissing-resource\t\tres://a.tscn\tgone\n");
        assert_eq!(known.len(), 1);
    }

    #[test]
    fn the_file_is_sorted_so_it_diffs_cleanly() {
        let items = vec![
            f("unknown-uid", "res://z.tscn", "z", 1),
            f("missing-resource", "res://a.tscn", "a", 1),
        ];
        let text = render(&items, "2026-09-22");
        let body: Vec<&str> = text.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(body.len(), 2);
        assert!(body[0].starts_with("missing-resource"));
    }
}
