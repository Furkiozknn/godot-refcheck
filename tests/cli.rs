//! Command-line behaviour: exit codes, output formats and argument handling.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_godot-refcheck")
}

fn project(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/projects").join(name)
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str]) -> Out {
    let o = Command::new(bin()).args(args).output().expect("failed to run the binary");
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).to_string(),
        stderr: String::from_utf8_lossy(&o.stderr).to_string(),
    }
}

// --- a small JSON validator, so escaping bugs cannot pass unnoticed ---------

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && (self.b[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }
    fn value(&mut self) -> Result<(), String> {
        self.ws();
        match self.b.get(self.i) {
            None => Err("unexpected end".into()),
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string(),
            Some(b't') => self.lit("true"),
            Some(b'f') => self.lit("false"),
            Some(b'n') => self.lit("null"),
            Some(_) => self.number(),
        }
    }
    fn lit(&mut self, s: &str) -> Result<(), String> {
        if self.b[self.i..].starts_with(s.as_bytes()) {
            self.i += s.len();
            Ok(())
        } else {
            Err(format!("expected {}", s))
        }
    }
    fn number(&mut self) -> Result<(), String> {
        let start = self.i;
        while self.i < self.b.len()
            && matches!(self.b[self.i], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')
        {
            self.i += 1;
        }
        if self.i == start {
            return Err(format!("not a value at byte {}", self.i));
        }
        Ok(())
    }
    fn string(&mut self) -> Result<(), String> {
        self.i += 1;
        while self.i < self.b.len() {
            match self.b[self.i] {
                b'\\' => self.i += 2,
                b'"' => {
                    self.i += 1;
                    return Ok(());
                }
                c if c < 0x20 => return Err("raw control character in string".into()),
                _ => self.i += 1,
            }
        }
        Err("unterminated string".into())
    }
    fn object(&mut self) -> Result<(), String> {
        self.i += 1;
        self.ws();
        if self.b.get(self.i) == Some(&b'}') {
            self.i += 1;
            return Ok(());
        }
        loop {
            self.ws();
            self.string()?;
            self.ws();
            if self.b.get(self.i) != Some(&b':') {
                return Err("expected :".into());
            }
            self.i += 1;
            self.value()?;
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(());
                }
                _ => return Err("expected , or }".into()),
            }
        }
    }
    fn array(&mut self) -> Result<(), String> {
        self.i += 1;
        self.ws();
        if self.b.get(self.i) == Some(&b']') {
            self.i += 1;
            return Ok(());
        }
        loop {
            self.value()?;
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(());
                }
                _ => return Err("expected , or ]".into()),
            }
        }
    }
}

fn assert_valid_json(s: &str) {
    let mut p = P { b: s.as_bytes(), i: 0 };
    p.value().unwrap_or_else(|e| panic!("invalid JSON: {}\n{}", e, s));
    p.ws();
    assert_eq!(p.i, s.len(), "trailing data after the JSON document");
}

// --- tests -----------------------------------------------------------------

#[test]
fn a_clean_project_exits_zero() {
    let o = run(&[project("clean").to_str().unwrap()]);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    assert!(o.stdout.contains("no problems found"));
}

#[test]
fn errors_exit_one_by_default() {
    let o = run(&[project("broken").to_str().unwrap()]);
    assert_eq!(o.code, 1);
}

#[test]
fn fail_on_never_still_prints_but_exits_zero() {
    let o = run(&[project("broken").to_str().unwrap(), "--fail-on", "never"]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("missing-resource"));
}

#[test]
fn fail_on_warning_catches_the_warning_only_project() {
    let clean = project("clean");
    let o = run(&[clean.to_str().unwrap(), "--fail-on", "warning"]);
    assert_eq!(o.code, 0);
    let o = run(&[project("broken").to_str().unwrap(), "--fail-on", "warning"]);
    assert_eq!(o.code, 1);
}

#[test]
fn json_output_is_valid_json() {
    let o = run(&[project("broken").to_str().unwrap(), "--json", "--fail-on", "never"]);
    assert_valid_json(&o.stdout);
    assert!(o.stdout.contains("\"check\": \"missing-resource\""));
    assert!(o.stdout.contains("\"located\""));
}

#[test]
fn json_output_of_a_clean_project_is_valid_json() {
    let o = run(&[project("clean").to_str().unwrap(), "--json"]);
    assert_valid_json(&o.stdout);
}

#[test]
fn sarif_output_is_valid_json_and_names_every_rule() {
    let dir = std::env::temp_dir().join(format!("refcheck-sarif-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("report.sarif");
    let o = run(&[
        project("broken").to_str().unwrap(),
        "--sarif",
        out.to_str().unwrap(),
        "--fail-on",
        "never",
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let text = std::fs::read_to_string(&out).unwrap();
    assert_valid_json(&text);
    assert!(text.contains("\"version\": \"2.1.0\""));
    for rule in godot_refcheck::finding::CHECKS {
        assert!(text.contains(rule.id), "rule {} missing from the SARIF run", rule.id);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_path_outside_a_project_is_a_usage_error() {
    let o = run(&[std::env::temp_dir().to_str().unwrap()]);
    assert_eq!(o.code, 2);
    assert!(o.stderr.contains("project.godot"));
}

#[test]
fn an_unknown_option_is_a_usage_error() {
    let o = run(&["--nope"]);
    assert_eq!(o.code, 2);
    assert!(o.stderr.contains("unknown option"));
}

#[test]
fn an_unknown_check_name_is_a_usage_error() {
    let o = run(&[project("clean").to_str().unwrap(), "--only", "not-a-check"]);
    assert_eq!(o.code, 2);
    assert!(o.stderr.contains("unknown check"));
}

#[test]
fn help_and_version_exit_zero() {
    assert_eq!(run(&["--help"]).code, 0);
    let v = run(&["--version"]);
    assert_eq!(v.code, 0);
    assert!(v.stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn list_checks_prints_every_check() {
    let o = run(&["--list-checks"]);
    assert_eq!(o.code, 0);
    for rule in godot_refcheck::finding::CHECKS {
        assert!(o.stdout.contains(rule.id));
    }
}

#[test]
fn recursive_mode_visits_every_fixture_project() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/projects");
    let o = run(&[root.to_str().unwrap(), "--recursive", "--fail-on", "never", "--quiet"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.contains("4 projects"), "{}", o.stdout);
}

#[test]
fn recursive_findings_name_the_project_they_came_from() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/projects");
    let o = run(&[root.to_str().unwrap(), "--recursive", "--fail-on", "never", "--json"]);
    assert_valid_json(&o.stdout);
    assert!(o.stdout.contains("broken/main.tscn"));
}

#[test]
fn the_project_file_itself_can_be_given_as_the_path() {
    let p = project("clean").join("project.godot");
    let o = run(&[p.to_str().unwrap()]);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
}

#[test]
fn a_subdirectory_resolves_up_to_the_project_root() {
    let p = project("clean").join("ui");
    let o = run(&[p.to_str().unwrap()]);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    assert!(o.stdout.contains("no problems found"));
}

#[test]
fn quiet_prints_one_line() {
    let o = run(&[project("broken").to_str().unwrap(), "--quiet", "--fail-on", "never"]);
    assert_eq!(o.stdout.lines().count(), 1);
}
