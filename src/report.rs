use crate::finding::{Finding, Level, CHECKS};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const HOMEPAGE: &str = "https://github.com/Furkiozknn/godot-refcheck";

pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

pub fn text(findings: &[Finding], scanned: usize, refs: usize, project: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("godot-refcheck {}  {}\n", VERSION, project));
    if findings.is_empty() {
        s.push_str(&format!("  {} files, {} references, no problems found.\n", scanned, refs));
        return s;
    }
    s.push('\n');
    for f in findings {
        s.push_str(&format!(
            "{}:{}: {}: {}: {}\n",
            f.located(),
            f.line,
            f.level.as_str(),
            f.check,
            f.message
        ));
        if !f.evidence.is_empty() {
            s.push_str(&format!("    {}\n", f.evidence));
        }
    }
    s.push_str(&format!("\n{}\n", summary(findings, 1, scanned, refs)));
    s
}

pub fn summary(findings: &[Finding], projects: usize, scanned: usize, refs: usize) -> String {
    let e = findings.iter().filter(|f| f.level == Level::Error).count();
    let w = findings.iter().filter(|f| f.level == Level::Warning).count();
    let i = findings.iter().filter(|f| f.level == Level::Info).count();
    let head = if projects == 1 { String::new() } else { format!("{} projects, ", projects) };
    format!(
        "{}{} files, {} references: {} errors, {} warnings, {} notes.",
        head, scanned, refs, e, w, i
    )
}

pub fn json(
    findings: &[Finding],
    scanned: usize,
    refs: usize,
    project: &str,
    applied: &[crate::fix::Applied],
    baselined: usize,
) -> String {
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!("  \"tool\": \"godot-refcheck\",\n  \"version\": \"{}\",\n", VERSION));
    s.push_str(&format!("  \"project\": \"{}\",\n", esc(project)));
    s.push_str(&format!("  \"files_scanned\": {},\n  \"references\": {},\n", scanned, refs));
    s.push_str(&format!("  \"baselined\": {},\n", baselined));
    s.push_str(&fixes_json(applied));
    s.push_str("  \"findings\": [\n");
    for (n, f) in findings.iter().enumerate() {
        s.push_str("    {");
        s.push_str(&format!("\"check\": \"{}\", ", f.check));
        s.push_str(&format!("\"level\": \"{}\", ", f.level.as_str()));
        s.push_str(&format!("\"project\": \"{}\", ", esc(&f.project)));
        s.push_str(&format!("\"file\": \"{}\", ", esc(&f.file)));
        s.push_str(&format!("\"located\": \"{}\", ", esc(&f.located())));
        s.push_str(&format!("\"line\": {}, ", f.line));
        s.push_str(&format!("\"message\": \"{}\", ", esc(&f.message)));
        s.push_str(&format!("\"evidence\": \"{}\"", esc(&f.evidence)));
        s.push('}');
        if n + 1 < findings.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ]\n}\n");
    s
}

pub fn sarif(findings: &[Finding]) -> String {
    let mut s = String::new();
    s.push_str("{\n  \"version\": \"2.1.0\",\n");
    s.push_str("  \"$schema\": \"https://json.schemastore.org/sarif-2.1.0.json\",\n");
    s.push_str("  \"runs\": [\n    {\n      \"tool\": {\n        \"driver\": {\n");
    s.push_str("          \"name\": \"godot-refcheck\",\n");
    s.push_str(&format!("          \"version\": \"{}\",\n", VERSION));
    s.push_str(&format!("          \"informationUri\": \"{}\",\n", HOMEPAGE));
    s.push_str("          \"rules\": [\n");
    for (n, c) in CHECKS.iter().enumerate() {
        s.push_str(&format!(
            "            {{\"id\": \"{}\", \"name\": \"{}\", \"shortDescription\": {{\"text\": \"{}\"}}, \"defaultConfiguration\": {{\"level\": \"{}\"}}}}",
            c.id,
            c.id,
            esc(c.summary),
            sarif_level(c.level)
        ));
        if n + 1 < CHECKS.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("          ]\n        }\n      },\n      \"results\": [\n");
    for (n, f) in findings.iter().enumerate() {
        s.push_str(&format!(
            "        {{\"ruleId\": \"{}\", \"level\": \"{}\", \"message\": {{\"text\": \"{}\"}}, \"locations\": [{{\"physicalLocation\": {{\"artifactLocation\": {{\"uri\": \"{}\"}}, \"region\": {{\"startLine\": {}}}}}}}]}}",
            f.check,
            sarif_level(f.level),
            esc(&if f.evidence.is_empty() {
                f.message.clone()
            } else {
                format!("{} ({})", f.message, f.evidence)
            }),
            esc(&f.located()),
            f.line.max(1)
        ));
        if n + 1 < findings.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("      ]\n    }\n  ]\n}\n");
    s
}

fn sarif_level(l: Level) -> &'static str {
    match l {
        Level::Error => "error",
        Level::Warning => "warning",
        Level::Info => "note",
    }
}

/// Today in ISO form, from the system clock, without pulling in a date crate.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs / 86_400;
    let (y, m, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Days since 1970-01-01 to a calendar date (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn fixes(applied: &[crate::fix::Applied], dry: bool) -> String {
    if applied.is_empty() {
        return String::new();
    }
    let mut s = String::new();
    let done = applied.iter().filter(|a| a.ok).count();
    s.push_str(&format!(
        "{} {} reference{}\n",
        if dry { "would repair" } else { "repaired" },
        done,
        if done == 1 { "" } else { "s" }
    ));
    for a in applied {
        let rel = a.fix.file.strip_prefix("res://").unwrap_or(&a.fix.file);
        if a.ok {
            s.push_str(&format!(
                "  {}:{}  {} -> {}\n      {} ({})\n",
                rel, a.fix.line, a.fix.old, a.fix.new, a.fix.reason, a.fix.check
            ));
        } else {
            s.push_str(&format!("  {}:{}  not repaired: {}\n", rel, a.fix.line, a.note));
        }
    }
    s.push('\n');
    s
}

pub fn fixes_json(applied: &[crate::fix::Applied]) -> String {
    let mut s = String::new();
    s.push_str("  \"repairs\": [\n");
    for (n, a) in applied.iter().enumerate() {
        s.push_str(&format!(
            "    {{\"check\": \"{}\", \"file\": \"{}\", \"line\": {}, \"old\": \"{}\", \"new\": \"{}\", \"applied\": {}, \"note\": \"{}\"}}",
            a.fix.check,
            esc(&a.fix.file),
            a.fix.line,
            esc(&a.fix.old),
            esc(&a.fix.new),
            a.ok,
            esc(&a.note)
        ));
        if n + 1 < applied.len() {
            s.push(',');
        }
        s.push('\n');
    }
    s.push_str("  ],\n");
    s
}
