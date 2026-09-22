//! Tokenizer for Godot's config-file family: `project.godot`, `.tscn`, `.tres`,
//! `.import`, `.cfg`, `export_presets.cfg`.
//!
//! The format looks like INI but values may span several physical lines
//! (arrays, dictionaries, constructor calls), and section headers carry inline
//! attributes. Splitting on `\n` is therefore wrong; we split on newlines that
//! occur at bracket depth zero and outside a string literal.

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assign {
    pub key: String,
    /// Raw right-hand side, exactly as written (may contain newlines).
    pub value: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Section(Section),
    Assign(Assign),
}

struct Logical {
    line: usize,
    text: String,
}

/// Split into logical lines, honouring string literals and bracket nesting.
fn logical_lines(src: &str) -> Vec<Logical> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut start = 1usize;
    let mut line = 1usize;
    let mut depth: i32 = 0;
    let mut in_str = false;
    let mut esc = false;
    for c in src.chars() {
        if c == '\r' {
            continue;
        }
        if c == '\n' {
            line += 1;
            if depth <= 0 && !in_str {
                if !buf.trim().is_empty() {
                    out.push(Logical { line: start, text: buf.clone() });
                }
                buf.clear();
                start = line;
                continue;
            }
            buf.push('\n');
            continue;
        }
        if in_str {
            buf.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                buf.push(c);
            }
            '[' | '(' | '{' => {
                depth += 1;
                buf.push(c);
            }
            ']' | ')' | '}' => {
                depth -= 1;
                buf.push(c);
            }
            _ => buf.push(c),
        }
    }
    if !buf.trim().is_empty() {
        out.push(Logical { line: start, text: buf });
    }
    out
}

/// Contents of every double-quoted literal in `s`, unescaped.
pub fn string_literals(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut esc = false;
    for c in s.chars() {
        if in_str {
            if esc {
                cur.push(match c {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    other => other,
                });
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
                out.push(std::mem::take(&mut cur));
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            in_str = true;
            cur.clear();
        }
    }
    out
}

fn split_header(body: &str) -> Vec<String> {
    // Split a section header body on whitespace that is outside strings and
    // outside brackets. Godot 3 writes `instance=ExtResource( 1 )` with spaces
    // inside the parentheses, so depth has to be tracked here as well.
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut esc = false;
    let mut depth = 0i32;
    for c in body.chars() {
        if in_str {
            cur.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        if c == '"' {
            in_str = true;
            cur.push(c);
        } else if c == '(' || c == '[' || c == '{' {
            depth += 1;
            cur.push(c);
        } else if c == ')' || c == ']' || c == '}' {
            depth -= 1;
            cur.push(c);
        } else if c.is_whitespace() && depth == 0 {
            if !cur.is_empty() {
                parts.push(std::mem::take(&mut cur));
            }
        } else if !c.is_whitespace() || depth > 0 {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    parts
}

pub fn unquote(v: &str) -> String {
    let v = v.trim();
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        string_literals(v).into_iter().next().unwrap_or_default()
    } else {
        v.to_string()
    }
}

pub fn parse(src: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for l in logical_lines(src) {
        let t = l.text.trim();
        if t.is_empty() || t.starts_with(';') || t.starts_with("//") {
            continue;
        }
        if t.starts_with('[') && t.ends_with(']') {
            let body = &t[1..t.len() - 1];
            let parts = split_header(body);
            if parts.is_empty() {
                continue;
            }
            let name = parts[0].clone();
            let mut attrs = Vec::new();
            for p in &parts[1..] {
                if let Some(eq) = p.find('=') {
                    attrs.push((p[..eq].to_string(), unquote(&p[eq + 1..])));
                }
            }
            out.push(Entry::Section(Section { name, attrs, line: l.line }));
            continue;
        }
        if let Some(eq) = first_top_level_eq(t) {
            let key = t[..eq].trim().to_string();
            let value = t[eq + 1..].trim().to_string();
            if !key.is_empty() && key.chars().all(|c| !c.is_whitespace()) {
                out.push(Entry::Assign(Assign { key, value, line: l.line }));
            }
        }
    }
    out
}

fn first_top_level_eq(s: &str) -> Option<usize> {
    let mut in_str = false;
    let mut esc = false;
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            '=' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Line number of a byte offset in `src` (1-based).
pub fn line_of(src: &str, offset: usize) -> usize {
    src.as_bytes()[..offset.min(src.len())].iter().filter(|b| **b == b'\n').count() + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_header_with_inline_attributes() {
        let e = parse("[ext_resource type=\"Script\" path=\"res://a.gd\" id=\"1\"]\n");
        match &e[0] {
            Entry::Section(s) => {
                assert_eq!(s.name, "ext_resource");
                assert_eq!(s.attrs.len(), 3);
                assert_eq!(s.attrs[1], ("path".into(), "res://a.gd".into()));
            }
            _ => panic!("expected a section"),
        }
    }

    /// Godot 3 writes `instance=ExtResource( 1 )` with spaces inside the
    /// parentheses. Splitting the header on whitespace alone tore that apart and
    /// produced `ExtResource("")`, which then looked like an undeclared id in
    /// every Godot 3 scene of the KidsCanCode recipe collection.
    #[test]
    fn godot3_header_keeps_spaced_constructor_together() {
        let e = parse("[node name=\"A\" parent=\".\" instance=ExtResource( 3 )]\n");
        match &e[0] {
            Entry::Section(s) => {
                let v = s.attrs.iter().find(|(k, _)| k == "instance").unwrap();
                assert_eq!(v.1, "ExtResource( 3 )");
            }
            _ => panic!("expected a section"),
        }
    }

    #[test]
    fn value_may_span_several_physical_lines() {
        let src = "[resource]\n_data = {\n\"a\": 1,\n\"b\": 2\n}\nnext = 3\n";
        let e = parse(src);
        let assigns: Vec<_> = e
            .iter()
            .filter_map(|x| match x {
                Entry::Assign(a) => Some(a),
                _ => None,
            })
            .collect();
        assert_eq!(assigns.len(), 2);
        assert_eq!(assigns[0].key, "_data");
        assert!(assigns[0].value.contains("\"b\": 2"));
        assert_eq!(assigns[0].line, 2);
        assert_eq!(assigns[1].line, 6);
    }

    #[test]
    fn a_bracket_inside_a_string_does_not_open_a_section() {
        let e = parse("[node name=\"A\"]\ntext = \"[b]bold[/b]\"\n");
        assert_eq!(e.len(), 2);
        assert!(matches!(e[1], Entry::Assign(_)));
    }

    #[test]
    fn equals_inside_a_string_is_not_the_assignment() {
        let e = parse("text = \"a=b\"\n");
        match &e[0] {
            Entry::Assign(a) => {
                assert_eq!(a.key, "text");
                assert_eq!(a.value, "\"a=b\"");
            }
            _ => panic!("expected an assignment"),
        }
    }

    #[test]
    fn comment_lines_are_ignored() {
        let e = parse("; Engine configuration file.\nconfig_version=5\n");
        assert_eq!(e.len(), 1);
    }

    #[test]
    fn string_literals_are_unescaped() {
        let v = string_literals("PackedStringArray(\"a\\\"b\", \"c\")");
        assert_eq!(v, vec!["a\"b".to_string(), "c".to_string()]);
    }

    #[test]
    fn line_numbers_are_one_based() {
        assert_eq!(line_of("a\nb\nc", 0), 1);
        assert_eq!(line_of("a\nb\nc", 2), 2);
        assert_eq!(line_of("a\nb\nc", 4), 3);
    }

    #[test]
    fn multiline_string_value_does_not_break_the_next_key() {
        let src =
            "config/description=\"line one\nline two\"\nconfig/tags=PackedStringArray(\"demo\")\n";
        let e = parse(src);
        assert_eq!(e.len(), 2);
        match &e[1] {
            Entry::Assign(a) => assert_eq!(a.key, "config/tags"),
            _ => panic!("expected an assignment"),
        }
    }
}
