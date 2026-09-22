//! Minimal GDScript lexer: enough to find `preload()` / `load()` calls with a
//! literal argument, without being fooled by comments or by the word appearing
//! inside a string.

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Str { value: String, offset: usize },
    Punct(char),
}

pub fn lex(src: &str) -> Vec<Tok> {
    let b: Vec<char> = src.chars().collect();
    // Byte offset for each char index, so reported offsets index into `src`.
    let mut offsets = Vec::with_capacity(b.len() + 1);
    let mut acc = 0usize;
    for c in &b {
        offsets.push(acc);
        acc += c.len_utf8();
    }
    offsets.push(acc);

    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c == '#' {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            let start = i;
            let triple = i + 2 < b.len() && b[i + 1] == c && b[i + 2] == c;
            let quote = c;
            let mut value = String::new();
            if triple {
                i += 3;
                while i < b.len() {
                    if b[i] == quote && i + 2 < b.len() && b[i + 1] == quote && b[i + 2] == quote {
                        i += 3;
                        break;
                    }
                    value.push(b[i]);
                    i += 1;
                }
            } else {
                i += 1;
                while i < b.len() {
                    let ch = b[i];
                    if ch == '\\' && i + 1 < b.len() {
                        let n = b[i + 1];
                        value.push(match n {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            other => other,
                        });
                        i += 2;
                        continue;
                    }
                    if ch == quote {
                        i += 1;
                        break;
                    }
                    if ch == '\n' {
                        break; // unterminated single-line string
                    }
                    value.push(ch);
                    i += 1;
                }
            }
            out.push(Tok::Str { value, offset: offsets[start] });
            continue;
        }
        if c.is_alphanumeric() || c == '_' {
            let start = i;
            while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_') {
                i += 1;
            }
            // `r"..."` / `r'...'` raw-string prefix is not an identifier.
            let word: String = b[start..i].iter().collect();
            if word == "r" && i < b.len() && (b[i] == '"' || b[i] == '\'') {
                continue;
            }
            out.push(Tok::Ident(word));
            continue;
        }
        if !c.is_whitespace() {
            out.push(Tok::Punct(c));
        }
        i += 1;
    }
    out
}

/// `(byte offset, path)` for every `preload("…")` / `load("…")` whose argument
/// is a literal. Non-literal arguments are deliberately ignored.
pub fn resource_loads(src: &str) -> Vec<(usize, String)> {
    let toks = lex(src);
    let mut out = Vec::new();
    for w in toks.windows(4) {
        if let (Tok::Ident(name), Tok::Punct('('), Tok::Str { value, offset }, tail) =
            (&w[0], &w[1], &w[2], &w[3])
        {
            if name != "preload" && name != "load" {
                continue;
            }
            // The literal has to BE the argument. `load("res://x/%s.png" % n)`
            // and `load("res://dir/" + name)` are computed at run time and the
            // fragment on its own is not a path that should exist.
            match tail {
                Tok::Punct(')') | Tok::Punct(',') => out.push((*offset, value.clone())),
                _ => {}
            }
        }
    }
    out
}

/// Every string literal in the file (used only by the advisory unused-asset scan).
pub fn string_literals(src: &str) -> Vec<String> {
    lex(src)
        .into_iter()
        .filter_map(|t| match t {
            Tok::Str { value, .. } => Some(value),
            _ => None,
        })
        .collect()
}

/// `#include "…"` directives in a `.gdshader` / `.gdshaderinc` file.
pub fn shader_includes(src: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    for line in src.split_inclusive('\n') {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("#include") {
            if let Some(a) = rest.find('"') {
                if let Some(b) = rest[a + 1..].find('"') {
                    out.push((offset, rest[a + 1..a + 1 + b].to_string()));
                }
            }
        }
        offset += line.len();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_preload_is_found() {
        let v = resource_loads("const A = preload(\"res://a.tscn\")\n");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].1, "res://a.tscn");
    }

    #[test]
    fn load_with_a_second_argument_is_still_a_literal_path() {
        let v = resource_loads("var a = load(\"res://a.tscn\", \"PackedScene\")\n");
        assert_eq!(v.len(), 1);
    }

    /// Pixelorama builds paths at run time: `load("res://Translations/%s.po" % lang)`.
    /// Treating the format string as a path invented three findings in a project
    /// that has nothing wrong with it.
    #[test]
    fn format_string_argument_is_not_a_path() {
        assert!(resource_loads("load(\"res://Translations/%s.po\" % lang)\n").is_empty());
    }

    /// material-maker does `load("res://material_maker/theme" + name + ".tres")`.
    #[test]
    fn concatenated_argument_is_not_a_path() {
        assert!(resource_loads("var t = load(\"res://theme/\" + name + \".tres\")\n").is_empty());
    }

    #[test]
    fn a_commented_out_preload_is_not_a_reference() {
        assert!(resource_loads("# const A = preload(\"res://a.tscn\")\n").is_empty());
    }

    #[test]
    fn a_preload_inside_a_string_is_not_a_reference() {
        assert!(resource_loads("var s = \"preload(\\\"res://a.tscn\\\")\"\n").is_empty());
    }

    #[test]
    fn triple_quoted_block_is_one_string() {
        let v = resource_loads("var doc = \"\"\"\npreload(\"res://a.tscn\")\n\"\"\"\nvar b = preload(\"res://b.tscn\")\n");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].1, "res://b.tscn");
    }

    #[test]
    fn single_quoted_strings_work_too() {
        let v = resource_loads("var a = preload('res://a.tscn')\n");
        assert_eq!(v[0].1, "res://a.tscn");
    }

    #[test]
    fn uid_argument_is_reported_verbatim() {
        let v = resource_loads("var a = preload(\"uid://cabc\")\n");
        assert_eq!(v[0].1, "uid://cabc");
    }

    #[test]
    fn method_named_load_on_another_object_still_counts_when_literal() {
        let v = resource_loads("ResourceLoader.load(\"res://a.tscn\")\n");
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn an_identifier_ending_in_load_is_not_load() {
        assert!(resource_loads("preload_scene(\"res://a.tscn\")\n").is_empty());
        assert!(resource_loads("reload(\"res://a.tscn\")\n").is_empty());
    }

    #[test]
    fn shader_includes_are_extracted() {
        let v = shader_includes("shader_type canvas_item;\n#include \"base.gdshaderinc\"\n");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].1, "base.gdshaderinc");
    }

    #[test]
    fn escape_sequences_do_not_end_a_string_early() {
        let v = resource_loads("var s = \"a\\\"b\"\nvar a = preload(\"res://a.tscn\")\n");
        assert_eq!(v.len(), 1);
    }
}
