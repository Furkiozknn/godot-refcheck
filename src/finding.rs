use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Info,
    Warning,
    Error,
}

impl Level {
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warning => "warning",
            Level::Info => "info",
        }
    }
    pub fn parse(s: &str) -> Option<Level> {
        match s.to_ascii_lowercase().as_str() {
            "error" => Some(Level::Error),
            "warning" | "warn" => Some(Level::Warning),
            "info" | "note" => Some(Level::Info),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub project: String,
    pub check: &'static str,
    pub level: Level,
    pub file: String,
    pub line: usize,
    pub message: String,
    pub evidence: String,
}

impl Finding {
    pub fn sort_key(&self) -> (std::cmp::Reverse<Level>, String, String, usize, &'static str) {
        (
            std::cmp::Reverse(self.level),
            self.project.clone(),
            self.file.clone(),
            self.line,
            self.check,
        )
    }

    /// Path of the offending file relative to the scan root, for editors and SARIF.
    pub fn located(&self) -> String {
        let rel = self.file.strip_prefix("res://").unwrap_or(&self.file);
        if self.project.is_empty() {
            rel.to_string()
        } else {
            format!("{}/{}", self.project.trim_end_matches('/'), rel)
        }
    }
}

pub fn sort(findings: &mut [Finding]) {
    findings.sort_by(|a, b| match a.sort_key().partial_cmp(&b.sort_key()) {
        Some(o) => o,
        None => Ordering::Equal,
    });
}

pub struct CheckInfo {
    pub id: &'static str,
    pub level: Level,
    pub summary: &'static str,
}

pub const CHECKS: &[CheckInfo] = &[
    CheckInfo {
        id: "missing-resource",
        level: Level::Error,
        summary: "a referenced res:// file is not in the project",
    },
    CheckInfo {
        id: "case-mismatch",
        level: Level::Error,
        summary: "a reference differs from the file on disk only by letter case",
    },
    CheckInfo {
        id: "unknown-uid",
        level: Level::Error,
        summary: "a uid:// reference matches no file and has no usable path fallback",
    },
    CheckInfo {
        id: "duplicate-uid",
        level: Level::Error,
        summary: "two files claim the same uid://",
    },
    CheckInfo {
        id: "undeclared-id",
        level: Level::Error,
        summary: "ExtResource()/SubResource() uses an id the file never declares",
    },
    CheckInfo {
        id: "broken-connection",
        level: Level::Error,
        summary: "a signal connection points at a node the scene does not contain",
    },
    CheckInfo {
        id: "missing-node-path",
        level: Level::Error,
        summary: "a script reaches for $Node/Path that the scene it runs in does not contain",
    },
    CheckInfo {
        id: "duplicate-class-name",
        level: Level::Error,
        summary: "two scripts declare the same global class_name",
    },
    CheckInfo {
        id: "uid-path-mismatch",
        level: Level::Warning,
        summary: "uid:// and path in the same reference point at different files",
    },
    CheckInfo {
        id: "stale-import",
        level: Level::Warning,
        summary: "a .import file is left over from a deleted asset",
    },
    CheckInfo {
        id: "unused-asset",
        level: Level::Info,
        summary: "an asset nothing in the project appears to reference (opt-in)",
    },
];
