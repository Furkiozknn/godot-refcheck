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

pub fn json(findings: &[Finding], scanned: usize, refs: usize, project: &str) -> String {
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!("  \"tool\": \"godot-refcheck\",\n  \"version\": \"{}\",\n", VERSION));
    s.push_str(&format!("  \"project\": \"{}\",\n", esc(project)));
    s.push_str(&format!("  \"files_scanned\": {},\n  \"references\": {},\n", scanned, refs));
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
