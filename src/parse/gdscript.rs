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

/// `(byte offset, node path)` for every node path a script claims exists:
/// the bare `$Head/Body` form and `get_node("Head/Body")` with a literal.
///
/// WHY THIS IS HERE. A `.tscn` writes its signal connections down and
/// `checks.rs` resolves those against the scene tree. Most projects do not
/// connect in the editor at all - the four games in this ecosystem have **no**
/// `[connection]` blocks and 145 `.connect(` call sites in GDScript instead -
/// so the resolver was pointed at a file section those projects never write.
/// `$Head/Body` is the same claim as a connection's `to=`, and when it is
/// wrong the engine hands back `null` and the next line dies at run time, on
/// the branch that reaches it.
///
/// DELIBERATELY NOT COLLECTED, each for the same reason the rest of this tool
/// gives: a path it cannot see is not a path it may judge.
///   * `%UniqueName` - resolved by owner, not by path.
///   * `..`, `.` and absolute `/root/...` - they leave this scene.
///   * `get_node_or_null` and `has_node` - those exist to ASK, and a missing
///     path is the answer rather than a fault.
///   * `$"quoted"`, `get_node("a" + b)`, `%s` interpolation - built at run time.
pub fn node_paths(src: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();

    // --- bare `$Head/Body` -------------------------------------------------
    // Its own pass rather than the token stream: `lex` drops whitespace, and
    // `$`, `Head`, `/`, `Body` would arrive as four tokens with no way left to
    // tell `$Head/Body` from `$Head / Body`.
    let b: Vec<char> = src.chars().collect();
    let mut offsets = Vec::with_capacity(b.len() + 1);
    let mut acc = 0usize;
    for c in &b {
        offsets.push(acc);
        acc += c.len_utf8();
    }
    offsets.push(acc);

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
            let quote = c;
            let triple = i + 2 < b.len() && b[i + 1] == quote && b[i + 2] == quote;
            i += if triple { 3 } else { 1 };
            while i < b.len() {
                if !triple && b[i] == '\\' {
                    i += 2;
                    continue;
                }
                if triple {
                    if b[i] == quote && i + 2 < b.len() && b[i + 1] == quote && b[i + 2] == quote {
                        i += 3;
                        break;
                    }
                } else {
                    if b[i] == quote {
                        i += 1;
                        break;
                    }
                    if b[i] == '\n' {
                        break;
                    }
                }
                i += 1;
            }
            continue;
        }
        if c == '$' {
            let start = i + 1;
            let mut j = start;
            while j < b.len()
                && (b[j].is_alphanumeric() || b[j] == '_' || b[j] == '/' || b[j] == '-')
            {
                j += 1;
            }
            if j > start {
                let raw: String = b[start..j].iter().collect();
                if let Some(path) = normalise_node_path(&raw) {
                    out.push((offsets[i], path));
                }
            }
            i = j.max(i + 1);
            continue;
        }
        i += 1;
    }

    // --- `get_node("Head/Body")` ------------------------------------------
    // The token window refuses a computed argument the same way
    // `resource_loads` does, and the RECEIVER has to be this script.
    //
    // `oyun.get_node("Dunya/Oyuncu")` asks another node, and its path is
    // written against THAT node's scene. Ignoring the receiver produced 132
    // findings on one shipped game - every one of them a test harness
    // reaching into a scene it had just instantiated. A check that loud is
    // not a check.
    let toks = lex(src);
    for (i, w) in toks.windows(4).enumerate() {
        if let (Tok::Ident(name), Tok::Punct('('), Tok::Str { value, offset }, tail) =
            (&w[0], &w[1], &w[2], &w[3])
        {
            if name != "get_node" {
                continue;
            }
            // Preceded by `.`: only `self.get_node(...)` is this script.
            if i > 0 && toks[i - 1] == Tok::Punct('.') {
                let is_self = i > 1 && toks[i - 2] == Tok::Ident("self".to_string());
                if !is_self {
                    continue;
                }
            }
            match tail {
                Tok::Punct(')') | Tok::Punct(',') => {
                    if let Some(path) = normalise_node_path(value) {
                        out.push((*offset, path));
                    }
                }
                _ => {}
            }
        }
    }

    out.sort();
    out.dedup();
    out
}

/// Names a script gives to nodes it creates: `alet.name = "Aletler"`.
///
/// A node added with `add_child` at run time is in no `.tscn`, so a path
/// reaching it looks missing to any static tool. This is the same class of
/// blindness README "Limitations" already states for computed paths, and the
/// cheapest sound way to stay quiet about it: if some script in the project
/// names a node exactly the way the missing segment is spelled, that node may
/// well be built at run time and nothing here can prove otherwise.
pub fn assigned_node_names(src: &str) -> Vec<String> {
    let toks = lex(src);
    let mut out = Vec::new();
    for w in toks.windows(3) {
        if let (Tok::Ident(key), Tok::Punct('='), Tok::Str { value, .. }) = (&w[0], &w[1], &w[2]) {
            if key == "name" && !value.is_empty() {
                out.push(value.clone());
            }
        }
    }
    out
}

/// A node path this tool is willing to judge, normalised; `None` otherwise.
fn normalise_node_path(raw: &str) -> Option<String> {
    let t = raw.trim();
    let t = t.strip_prefix("./").unwrap_or(t);
    let t = t.trim_end_matches('/');
    if t.is_empty() || t.starts_with('/') || t.contains('%') {
        return None;
    }
    if t.split('/').any(|s| s == ".." || s == "." || s.is_empty()) {
        return None;
    }
    Some(t.to_string())
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

/// `class_name X` declared at the top level of a script.
pub fn class_name(src: &str) -> Option<(usize, String)> {
    let toks = lex(src);
    for w in toks.windows(2) {
        if let (Tok::Ident(kw), Tok::Ident(name)) = (&w[0], &w[1]) {
            if kw == "class_name" {
                return Some((0, name.clone()));
            }
        }
    }
    None
}

/// `extends "res://base.gd"`. A bare `extends Node` names a type, not a file.
pub fn extends_path(src: &str) -> Option<(usize, String)> {
    let toks = lex(src);
    for w in toks.windows(2) {
        if let (Tok::Ident(kw), Tok::Str { value, offset }) = (&w[0], &w[1]) {
            if kw == "extends" {
                return Some((*offset, value.clone()));
            }
        }
    }
    None
}

/// `@icon("res://…")` above a class declaration.
pub fn icon_annotation(src: &str) -> Option<(usize, String)> {
    let toks = lex(src);
    for w in toks.windows(4) {
        if let (Tok::Punct('@'), Tok::Ident(name), Tok::Punct('('), Tok::Str { value, offset }) =
            (&w[0], &w[1], &w[2], &w[3])
        {
            if name == "icon" {
                return Some((*offset, value.clone()));
            }
        }
    }
    None
}

/// Paths the script itself declares optional: `if has_node("TextBubbleLayer")`.
///
/// `has_node` is how GDScript asks "is this node here?", so a path that a
/// script tests before using is a path the author already knows may be
/// absent. Reporting it says nothing the code does not already say; the
/// engine returns null and the guarded branch is never entered.
///
/// Same receiver rule as `get_node`: `other.has_node(...)` asks a different
/// node and settles nothing about this one.
pub fn guarded_node_paths(src: &str) -> Vec<String> {
    let toks = lex(src);
    let mut out = Vec::new();
    for (i, w) in toks.windows(4).enumerate() {
        if let (Tok::Ident(name), Tok::Punct('('), Tok::Str { value, .. }, tail) =
            (&w[0], &w[1], &w[2], &w[3])
        {
            if name != "has_node" {
                continue;
            }
            if i > 0 && toks[i - 1] == Tok::Punct('.') {
                let is_self = i > 1 && toks[i - 2] == Tok::Ident("self".to_string());
                if !is_self {
                    continue;
                }
            }
            if matches!(tail, Tok::Punct(')') | Tok::Punct(',')) {
                if let Some(path) = normalise_node_path(value) {
                    out.push(path);
                }
            }
        }
    }
    out.sort();
    out.dedup();
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

    #[test]
    fn class_name_is_read() {
        assert_eq!(class_name("extends Node\nclass_name Hero\n").unwrap().1, "Hero");
        assert_eq!(class_name("class_name Hero extends Node\n").unwrap().1, "Hero");
        assert!(class_name("extends Node\n").is_none());
    }

    #[test]
    fn an_inner_class_is_not_a_global_class_name() {
        assert!(class_name("extends Node\nclass Inner:\n\tpass\n").is_none());
    }

    #[test]
    fn class_name_inside_a_string_or_comment_is_not_a_declaration() {
        assert!(class_name("# class_name Hero\n").is_none());
        assert!(class_name("var s = \"class_name Hero\"\n").is_none());
    }

    #[test]
    fn extends_with_a_path_is_a_file_reference() {
        assert_eq!(extends_path("extends \"res://base.gd\"\n").unwrap().1, "res://base.gd");
        assert!(extends_path("extends CharacterBody2D\n").is_none());
    }

    #[test]
    fn icon_annotation_is_a_file_reference() {
        let v = icon_annotation("@icon(\"res://icons/hero.svg\")\nextends Node\n").unwrap();
        assert_eq!(v.1, "res://icons/hero.svg");
        assert!(icon_annotation("@export var x: int\n").is_none());
    }

    fn paths(src: &str) -> Vec<String> {
        node_paths(src).into_iter().map(|(_, p)| p).collect()
    }

    #[test]
    fn a_bare_dollar_path_is_a_node_reference() {
        assert_eq!(paths("func _ready(): $Head/Body.hide()\n"), ["Head/Body"]);
        assert_eq!(paths("$Button.pressed.connect(_go)\n"), ["Button"]);
    }

    #[test]
    fn the_path_stops_where_the_expression_does() {
        // `$A/B.c()` is the node A/B; the member access is not part of it.
        assert_eq!(paths("$A/B.visible = true\n"), ["A/B"]);
        assert_eq!(paths("if $A/B: pass\n"), ["A/B"]);
        assert_eq!(paths("var x = [$A, $B]\n"), ["A", "B"]);
    }

    #[test]
    fn a_dollar_in_a_comment_or_a_string_is_not_a_path() {
        assert!(paths("# $Head/Body is gone\n").is_empty());
        assert!(paths("var s = \"$Head/Body\"\n").is_empty());
        assert!(paths("var s = \"\"\"\n$Head\n\"\"\"\n").is_empty());
    }

    #[test]
    fn unique_names_and_parent_steps_are_out_of_scope() {
        // `%Name` is resolved by owner; `..` leaves the scene; an absolute
        // path addresses the live tree rather than this scene.
        assert!(paths("%Hero.hide()\n").is_empty());
        assert!(paths("$\"%Hero\".hide()\n").is_empty());
        assert!(paths("$../Sibling.hide()\n").is_empty());
        assert!(paths("get_node(\"/root/Main\").hide()\n").is_empty());
    }

    #[test]
    fn get_node_with_a_literal_is_a_node_reference() {
        assert_eq!(paths("get_node(\"Head/Body\").hide()\n"), ["Head/Body"]);
        assert_eq!(paths("self.get_node(\"Head\").hide()\n"), ["Head"]);
    }

    #[test]
    fn get_node_on_another_receiver_is_not_this_scenes_path() {
        // On one shipped game, ignoring the receiver produced 132 findings:
        // a test harness asking a scene it had just instantiated.
        assert!(paths("oyun.get_node(\"Dunya/Oyuncu\").hide()\n").is_empty());
        assert!(paths("get_tree().root.get_node(\"Main\").hide()\n").is_empty());
    }

    #[test]
    fn asking_whether_a_node_exists_is_not_a_claim_that_it_does() {
        assert!(paths("if has_node(\"Head\"): pass\n").is_empty());
        assert!(paths("var n = get_node_or_null(\"Head\")\n").is_empty());
    }

    #[test]
    fn has_node_names_the_paths_the_script_treats_as_optional() {
        assert_eq!(guarded_node_paths("if has_node(\"Bubble\"): pass\n"), ["Bubble"]);
        assert_eq!(guarded_node_paths("if self.has_node(\"A/B\"): pass\n"), ["A/B"]);
    }

    #[test]
    fn has_node_asked_of_another_node_guards_nothing_here() {
        // Same receiver rule as `get_node`: `other.has_node("X")` settles
        // whether X is under `other`, not under this script's node.
        assert!(guarded_node_paths("if oyun.has_node(\"X\"): pass\n").is_empty());
        assert!(guarded_node_paths("if has_node(name): pass\n").is_empty());
    }

    #[test]
    fn a_path_built_at_run_time_is_left_alone() {
        assert!(paths("get_node(\"levels/\" + name).hide()\n").is_empty());
        assert!(paths("get_node(\"lvl%s\" % n).hide()\n").is_empty());
    }

    #[test]
    fn the_same_path_twice_is_reported_once_per_place() {
        // Dedup is by (offset, path), so two different lines both count.
        assert_eq!(node_paths("$A.hide()\n$A.show()\n").len(), 2);
    }

    #[test]
    fn a_name_a_script_gives_a_node_is_collected() {
        let src = "var n := Node.new()\nn.name = \"Aletler\"\nadd_child(n)\n";
        assert_eq!(assigned_node_names(src), ["Aletler"]);
    }

    #[test]
    fn a_name_in_a_comment_is_not_collected() {
        assert!(assigned_node_names("# name = \"Aletler\"\n").is_empty());
        assert!(assigned_node_names("var x = 1\n").is_empty());
    }
}
