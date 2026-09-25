//! Project discovery: what files exist, what UIDs they declare, what they reference.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use crate::parse::cfgfile::{self, Entry};
use crate::parse::gdscript;
use crate::scene::{ConnDecl, ExtTarget, NodeDecl, SceneFile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    ExtResource,
    Preload,
    ProjectSetting,
    ShaderInclude,
    PluginScript,
    Extends,
    Icon,
}

impl RefKind {
    pub fn label(&self) -> &'static str {
        match self {
            RefKind::ExtResource => "ext_resource",
            RefKind::Preload => "preload/load",
            RefKind::ProjectSetting => "project setting",
            RefKind::ShaderInclude => "#include",
            RefKind::PluginScript => "plugin.cfg",
            RefKind::Extends => "extends",
            RefKind::Icon => "@icon",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Reference {
    pub from: String,
    pub line: usize,
    pub kind: RefKind,
    pub path: Option<String>,
    pub uid: Option<String>,
    pub detail: String,
    /// The literal exactly as it is written in the file, so a repair can put
    /// the corrected text back in its place without touching anything else.
    pub raw: String,
}

#[derive(Debug, Clone)]
pub struct UidDecl {
    pub uid: String,
    pub owner: String,
    pub from: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct IdUse {
    pub from: String,
    pub line: usize,
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Default)]
pub struct Project {
    pub root: PathBuf,
    /// res:// paths present on disk.
    pub files: BTreeSet<String>,
    /// lowercase res:// path -> the real spelling on disk.
    pub lower: HashMap<String, String>,
    /// res:// paths that no importer writes to disk but the engine can still load
    /// (translations produced from a .csv, for example).
    pub generated: BTreeSet<String>,
    pub uid_owner: BTreeMap<String, String>,
    pub uid_decls: Vec<UidDecl>,
    pub refs: Vec<Reference>,
    pub id_uses: Vec<IdUse>,
    pub id_decls: BTreeSet<(String, String, String)>,
    /// res:// path -> every plain string literal found in it (advisory scan only).
    pub literals: BTreeMap<String, Vec<String>>,
    pub stale_imports: Vec<(String, usize, String)>,
    pub scenes: BTreeMap<String, SceneFile>,
    /// global `class_name` -> the scripts that declare it.
    pub class_names: BTreeMap<String, Vec<String>>,
    /// script res:// path -> the node paths it claims exist, with their lines.
    pub script_node_paths: BTreeMap<String, Vec<(usize, String)>>,
    /// Every name a script gives a node it creates (`x.name = "Aletler"`).
    /// A node built at run time is in no scene file; see `assigned_node_names`.
    pub runtime_node_names: BTreeSet<String>,
    /// The root node name of every scene in the project.
    ///
    /// `add_child(preload("res://x.tscn").instantiate())` gives the new child
    /// the name of THAT scene's root node, and the call site usually cannot be
    /// tied to a particular scene statically - it arrives as a `PackedScene`
    /// parameter, or through an exported array. So a missing segment spelled
    /// like some scene's root is a node that may well be there at run time.
    pub scene_root_names: BTreeSet<String>,
    /// script res:// path -> the node paths it guards with `has_node(...)`.
    pub script_guarded_paths: BTreeMap<String, BTreeSet<String>>,
    /// source asset -> the res:// paths its importer produces outside `.godot/`.
    pub import_products: BTreeMap<String, Vec<String>>,
    pub unreadable: Vec<String>,
    pub godot_version: Option<String>,
}

const SKIP_DIRS: &[&str] = &[".godot", ".git", ".import", ".svn", ".hg", "node_modules", ".vs"];

pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let p = if start.is_file() { start.parent()?.to_path_buf() } else { start.to_path_buf() };
    if p.join("project.godot").is_file() {
        return Some(p);
    }
    let mut cur = p.as_path();
    while let Some(parent) = cur.parent() {
        if parent.join("project.godot").is_file() {
            return Some(parent.to_path_buf());
        }
        cur = parent;
    }
    None
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let rd = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let ft = match e.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            walk(&p, out);
        } else if ft.is_file() {
            out.push(p);
        }
    }
}

pub fn to_res(root: &Path, p: &Path) -> Option<String> {
    let rel = p.strip_prefix(root).ok()?;
    let mut s = String::from("res://");
    let mut first = true;
    for c in rel.components() {
        if !first {
            s.push('/');
        }
        first = false;
        s.push_str(&c.as_os_str().to_string_lossy());
    }
    Some(s)
}

/// Normalise a `res://` path: strip sub-resource suffixes, collapse `.`/`..`.
pub fn normalize_res(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if !raw.starts_with("res://") {
        return None;
    }
    let body = &raw[6..];
    // `scene.tscn::3` is a sub-resource and `voice.wav:es` is a locale remap;
    // neither changes which file on disk is meant, and `:` cannot occur in a
    // path segment on Windows or macOS anyway.
    let body = match body.find(':') {
        Some(i) => &body[..i],
        None => body,
    };
    let mut parts: Vec<&str> = Vec::new();
    for seg in body.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("res://{}", parts.join("/")))
}

/// Resolve a possibly relative reference against the directory of `from`.
pub fn resolve_relative(from: &str, raw: &str) -> Option<String> {
    if raw.starts_with("res://") {
        return normalize_res(raw);
    }
    if raw.contains("://") {
        return None;
    }
    let dir = match from.rfind('/') {
        Some(i) => &from[..i],
        None => "res:/",
    };
    normalize_res(&format!("{}/{}", dir, raw))
}

fn ext_of(res: &str) -> String {
    match res.rfind('.') {
        Some(i) if i > res.rfind('/').map(|s| s + 1).unwrap_or(0) => {
            res[i + 1..].to_ascii_lowercase()
        }
        _ => String::new(),
    }
}

/// Occurrences of `ExtResource(...)` / `SubResource(...)` outside string literals.
pub fn id_references(value: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = value.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut in_str = false;
    let mut esc = false;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
            i += 1;
            continue;
        }
        for name in ["ExtResource", "SubResource"] {
            let n: Vec<char> = name.chars().collect();
            if i + n.len() < chars.len() && chars[i..i + n.len()] == n[..] {
                let prev_ok = i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
                let mut j = i + n.len();
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                if prev_ok && j < chars.len() && chars[j] == '(' {
                    j += 1;
                    let mut arg = String::new();
                    let mut q = false;
                    while j < chars.len() {
                        let d = chars[j];
                        if d == '"' {
                            q = !q;
                            j += 1;
                            continue;
                        }
                        if d == ')' && !q {
                            break;
                        }
                        arg.push(d);
                        j += 1;
                    }
                    out.push((name.to_string(), arg.trim().to_string()));
                    i = j;
                    break;
                }
            }
        }
        i += 1;
    }
    out
}

fn is_text(bytes: &[u8]) -> bool {
    !bytes.iter().take(8000).any(|b| *b == 0)
}

impl Project {
    pub fn exists(&self, res: &str) -> bool {
        self.files.contains(res) || self.generated.contains(res)
    }

    /// Whether `res` names a directory that holds at least one file.
    pub fn is_dir(&self, res: &str) -> bool {
        let prefix = format!("{}/", res.trim_end_matches('/'));
        self.files.range(prefix.clone()..).next().is_some_and(|f| f.starts_with(&prefix))
    }

    pub fn case_variant(&self, res: &str) -> Option<&String> {
        let l = res.to_ascii_lowercase();
        match self.lower.get(&l) {
            Some(real) if real != res => Some(real),
            _ => None,
        }
    }

    pub fn load(root: &Path) -> Project {
        let mut pr = Project { root: root.to_path_buf(), ..Default::default() };
        let mut disk = Vec::new();
        walk(root, &mut disk);

        for p in &disk {
            if let Some(res) = to_res(root, p) {
                pr.lower.entry(res.to_ascii_lowercase()).or_insert_with(|| res.clone());
                pr.files.insert(res);
            }
        }

        for p in &disk {
            let res = match to_res(root, p) {
                Some(r) => r,
                None => continue,
            };
            let ext = ext_of(&res);
            let name = res.rsplit('/').next().unwrap_or("").to_string();
            let raw = match fs::read(p) {
                Ok(b) => b,
                Err(_) => {
                    pr.unreadable.push(res.clone());
                    continue;
                }
            };
            if !is_text(&raw) {
                continue;
            }
            let src = String::from_utf8_lossy(&raw).to_string();
            match ext.as_str() {
                "uid" => pr.read_uid_sidecar(&res, &src),
                "import" => pr.read_import(&res, &src),
                "tscn" | "tres" | "escn" | "scn" => pr.read_scene(&res, &src),
                "gd" | "cs" => pr.read_script(&res, &src),
                "gdshader" | "gdshaderinc" | "shader" => pr.read_shader(&res, &src),
                "cfg" if name == "plugin.cfg" => pr.read_plugin_cfg(&res, &src),
                _ => {}
            }
            if name == "project.godot" && res == "res://project.godot" {
                pr.read_project_godot(&res, &src);
            }
        }

        for d in &pr.uid_decls {
            pr.uid_owner.entry(d.uid.clone()).or_insert_with(|| d.owner.clone());
        }
        pr
    }

    fn read_uid_sidecar(&mut self, res: &str, src: &str) {
        let uid = src.trim();
        if !uid.starts_with("uid://") {
            return;
        }
        let owner = res.trim_end_matches(".uid").to_string();
        self.uid_decls.push(UidDecl {
            uid: uid.to_string(),
            owner,
            from: res.to_string(),
            line: 1,
        });
    }

    fn read_import(&mut self, res: &str, src: &str) {
        let source_res = res.trim_end_matches(".import").to_string();
        let mut section = String::new();
        for e in cfgfile::parse(src) {
            match e {
                Entry::Section(s) => section = s.name,
                Entry::Assign(a) => match a.key.as_str() {
                    "uid" => {
                        let uid = cfgfile::unquote(&a.value);
                        if uid.starts_with("uid://") {
                            self.uid_decls.push(UidDecl {
                                uid,
                                owner: source_res.clone(),
                                from: res.to_string(),
                                line: a.line,
                            });
                        }
                    }
                    "dest_files" | "files" if section == "deps" || section == "remap" => {
                        for lit in cfgfile::string_literals(&a.value) {
                            if let Some(n) = normalize_res(&lit) {
                                if !n.starts_with("res://.godot/") {
                                    self.generated.insert(n.clone());
                                    self.import_products
                                        .entry(source_res.clone())
                                        .or_default()
                                        .push(n);
                                }
                            }
                        }
                    }
                    _ => {}
                },
            }
        }
        if !self.files.contains(&source_res) {
            self.stale_imports.push((res.to_string(), 1, source_res));
        }
    }

    fn read_scene(&mut self, res: &str, src: &str) {
        let mut scene = SceneFile::default();
        let mut seen_root = false;
        let mut current_node: Option<String> = None;
        for e in cfgfile::parse(src) {
            match e {
                Entry::Section(s) => {
                    let get = |k: &str| -> Option<String> {
                        s.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone())
                    };
                    // Any section that is not a [node] ends the previous
                    // one: a `script =` under [sub_resource] belongs to that
                    // resource, not to the node written above it.
                    if s.name != "node" {
                        current_node = None;
                    }
                    match s.name.as_str() {
                        "gd_scene" | "gd_resource" => {
                            if let Some(uid) = get("uid") {
                                if uid.starts_with("uid://") {
                                    self.uid_decls.push(UidDecl {
                                        uid,
                                        owner: res.to_string(),
                                        from: res.to_string(),
                                        line: s.line,
                                    });
                                }
                            }
                        }
                        "ext_resource" => {
                            let written = get("path");
                            let path = written.as_deref().and_then(normalize_res);
                            let uid = get("uid").filter(|u| u.starts_with("uid://"));
                            if let Some(id) = get("id") {
                                self.id_decls.insert((
                                    res.to_string(),
                                    "ExtResource".to_string(),
                                    id.trim().to_string(),
                                ));
                                scene.ext.insert(
                                    id.trim().to_string(),
                                    ExtTarget { path: path.clone(), uid: uid.clone() },
                                );
                            }
                            if path.is_some() || uid.is_some() {
                                self.refs.push(Reference {
                                    from: res.to_string(),
                                    line: s.line,
                                    kind: RefKind::ExtResource,
                                    path,
                                    uid,
                                    detail: get("type").unwrap_or_default(),
                                    raw: written.unwrap_or_default(),
                                });
                            }
                        }
                        "node" => {
                            let name = get("name").unwrap_or_default();
                            let parent = get("parent");
                            let is_root = parent.is_none() && !seen_root;
                            if is_root {
                                seen_root = true;
                            }
                            let instance = s
                                .attrs
                                .iter()
                                .find(|(k, _)| k == "instance")
                                .and_then(|(_, v)| {
                                    id_references(v).into_iter().find(|(k, _)| k == "ExtResource")
                                })
                                .map(|(_, id)| id);
                            let path = if is_root {
                                ".".to_string()
                            } else {
                                match parent.as_deref() {
                                    None | Some(".") => name.clone(),
                                    Some(p) => format!("{}/{}", p, name),
                                }
                            };
                            current_node = Some(path);
                            if is_root && !name.is_empty() {
                                self.scene_root_names.insert(name.clone());
                            }
                            scene.nodes.push(NodeDecl {
                                name,
                                parent: if is_root { None } else { parent },
                                instance,
                                placeholder: get("instance_placeholder").is_some(),
                                line: s.line,
                            });
                        }
                        "connection" => {
                            scene.connections.push(ConnDecl {
                                signal: get("signal").unwrap_or_default(),
                                from: get("from").unwrap_or_default(),
                                to: get("to").unwrap_or_default(),
                                method: get("method").unwrap_or_default(),
                                line: s.line,
                            });
                        }
                        "sub_resource" => {
                            if let Some(id) = get("id") {
                                self.id_decls.insert((
                                    res.to_string(),
                                    "SubResource".to_string(),
                                    id.trim().to_string(),
                                ));
                            }
                        }
                        _ => {}
                    }
                    // `instance=ExtResource("3")` lives in the header attributes.
                    for (_, v) in &s.attrs {
                        for (kind, id) in id_references(v) {
                            self.id_uses.push(IdUse {
                                from: res.to_string(),
                                line: s.line,
                                kind,
                                id,
                            });
                        }
                    }
                }
                Entry::Assign(a) => {
                    // `script = ExtResource("1_x")` inside a [node] block is
                    // what ties a .gd file to a scene. Without it a path
                    // written in that script has no tree to be resolved
                    // against, and the check below stays silent.
                    if a.key == "script" {
                        if let (Some(node), Some((_, id))) = (
                            current_node.clone(),
                            id_references(&a.value).into_iter().find(|(k, _)| k == "ExtResource"),
                        ) {
                            scene.scripts.insert(node, id);
                        }
                    }
                    for (kind, id) in id_references(&a.value) {
                        self.id_uses.push(IdUse { from: res.to_string(), line: a.line, kind, id });
                    }
                }
            }
        }
        if !scene.nodes.is_empty() || !scene.connections.is_empty() {
            self.scenes.insert(res.to_string(), scene);
        }
    }

    fn read_script(&mut self, res: &str, src: &str) {
        if res.ends_with(".gd") {
            self.runtime_node_names.extend(gdscript::assigned_node_names(src));
            let guarded = gdscript::guarded_node_paths(src);
            if !guarded.is_empty() {
                self.script_guarded_paths.insert(res.to_string(), guarded.into_iter().collect());
            }
            // One line can write the same path twice
            // (`$A/B.x = -$A/B.y`); that is one claim to judge, not two.
            let mut claims: Vec<(usize, String)> = gdscript::node_paths(src)
                .into_iter()
                .map(|(off, path)| (cfgfile::line_of(src, off), path))
                .collect();
            claims.sort();
            claims.dedup();
            if !claims.is_empty() {
                self.script_node_paths.insert(res.to_string(), claims);
            }
        }
        for (off, raw) in gdscript::resource_loads(src) {
            let line = cfgfile::line_of(src, off);
            if raw.starts_with("uid://") {
                self.refs.push(Reference {
                    from: res.to_string(),
                    line,
                    kind: RefKind::Preload,
                    path: None,
                    uid: Some(raw.clone()),
                    detail: String::new(),
                    raw,
                });
            } else if let Some(n) = normalize_res(&raw) {
                self.refs.push(Reference {
                    from: res.to_string(),
                    line,
                    kind: RefKind::Preload,
                    path: Some(n),
                    uid: None,
                    detail: String::new(),
                    raw,
                });
            }
        }
        if let Some((off, raw)) = gdscript::extends_path(src) {
            if let Some(n) = resolve_relative(res, &raw) {
                self.refs.push(Reference {
                    from: res.to_string(),
                    line: cfgfile::line_of(src, off),
                    kind: RefKind::Extends,
                    path: Some(n),
                    uid: None,
                    detail: "super class".into(),
                    raw,
                });
            }
        }
        if let Some((off, raw)) = gdscript::icon_annotation(src) {
            if let Some(n) = normalize_res(&raw) {
                self.refs.push(Reference {
                    from: res.to_string(),
                    line: cfgfile::line_of(src, off),
                    kind: RefKind::Icon,
                    path: Some(n),
                    uid: None,
                    detail: String::new(),
                    raw,
                });
            }
        }
        if let Some((_, name)) = gdscript::class_name(src) {
            self.class_names.entry(name).or_default().push(res.to_string());
        }
        self.literals.insert(res.to_string(), gdscript::string_literals(src));
    }

    fn read_shader(&mut self, res: &str, src: &str) {
        for (off, raw) in gdscript::shader_includes(src) {
            if let Some(n) = resolve_relative(res, &raw) {
                self.refs.push(Reference {
                    from: res.to_string(),
                    line: cfgfile::line_of(src, off),
                    kind: RefKind::ShaderInclude,
                    path: Some(n),
                    uid: None,
                    detail: String::new(),
                    raw,
                });
            }
        }
    }

    fn read_plugin_cfg(&mut self, res: &str, src: &str) {
        let mut section = String::new();
        for e in cfgfile::parse(src) {
            match e {
                Entry::Section(s) => section = s.name,
                Entry::Assign(a) => {
                    if section == "plugin" && a.key == "script" {
                        let raw = cfgfile::unquote(&a.value);
                        if let Some(n) = resolve_relative(res, &raw) {
                            self.refs.push(Reference {
                                from: res.to_string(),
                                line: a.line,
                                kind: RefKind::PluginScript,
                                path: Some(n),
                                uid: None,
                                detail: "script".into(),
                                raw,
                            });
                        }
                    }
                }
            }
        }
    }

    fn read_project_godot(&mut self, res: &str, src: &str) {
        let mut section = String::new();
        for e in cfgfile::parse(src) {
            match e {
                Entry::Section(s) => section = s.name,
                Entry::Assign(a) => {
                    if !ENGINE_SECTIONS.contains(&section.as_str()) {
                        continue;
                    }
                    if a.key == "config/features" {
                        let v = cfgfile::string_literals(&a.value);
                        self.godot_version = v.first().cloned();
                    }
                    for lit in cfgfile::string_literals(&a.value) {
                        let lit = lit.strip_prefix('*').unwrap_or(&lit).to_string();
                        if lit.starts_with("uid://") {
                            self.refs.push(Reference {
                                from: res.to_string(),
                                line: a.line,
                                kind: RefKind::ProjectSetting,
                                path: None,
                                uid: Some(lit.clone()),
                                detail: format!("{}{}", prefix(&section), a.key),
                                raw: lit,
                            });
                        } else if lit.starts_with("res://") {
                            if let Some(n) = normalize_res(&lit) {
                                self.refs.push(Reference {
                                    from: res.to_string(),
                                    line: a.line,
                                    kind: RefKind::ProjectSetting,
                                    path: Some(n),
                                    uid: None,
                                    detail: format!("{}{}", prefix(&section), a.key),
                                    raw: lit,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Sections the engine itself defines. A `res://` value in any other section is
/// left alone: Godot never loads it, so a missing target is not a real failure.
/// The `[locale]` block left behind by Godot 3 projects is the common case.
pub const ENGINE_SECTIONS: &[&str] = &[
    "",
    "application",
    "audio",
    "autoload",
    "collada",
    "compression",
    "debug",
    "display",
    "dotnet",
    "editor",
    "editor_plugins",
    "filesystem",
    "gdextension",
    "global_group",
    "gui",
    "importer_defaults",
    "input",
    "input_devices",
    "internationalization",
    "layer_names",
    "memory",
    "mono",
    "navigation",
    "network",
    "node",
    "physics",
    "rendering",
    "shader_globals",
    "threading",
    "world",
    "xr",
];

fn prefix(section: &str) -> String {
    if section.is_empty() {
        String::new()
    } else {
        format!("{}/", section)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_resource_suffix_is_stripped() {
        assert_eq!(normalize_res("res://a/b.tscn::3").unwrap(), "res://a/b.tscn");
    }

    /// `locale/translation_remaps` stores entries as `res://audio/hello_es.wav:es`.
    /// Reading the locale tag as part of the file name reported eleven missing
    /// files in the official Godot translation demo, where nothing is missing.
    #[test]
    fn locale_suffix_is_stripped() {
        assert_eq!(
            normalize_res("res://audio/hello_es.wav:es").unwrap(),
            "res://audio/hello_es.wav"
        );
    }

    #[test]
    fn dot_segments_collapse() {
        assert_eq!(normalize_res("res://a/./b/../c.png").unwrap(), "res://a/c.png");
    }

    #[test]
    fn non_res_schemes_are_not_paths() {
        assert!(normalize_res("user://save.cfg").is_none());
        assert!(normalize_res("uid://cabc").is_none());
        assert!(normalize_res("https://example.com/a.png").is_none());
    }

    #[test]
    fn relative_include_resolves_against_the_including_file() {
        assert_eq!(
            resolve_relative("res://shaders/use.gdshader", "base.gdshaderinc").unwrap(),
            "res://shaders/base.gdshaderinc"
        );
        assert_eq!(
            resolve_relative("res://a/b/c.gdshader", "../inc.gdshaderinc").unwrap(),
            "res://a/inc.gdshaderinc"
        );
    }

    #[test]
    fn absolute_reference_ignores_the_including_file() {
        assert_eq!(
            resolve_relative("res://a/b.gdshader", "res://c.gdshaderinc").unwrap(),
            "res://c.gdshaderinc"
        );
    }

    #[test]
    fn quoted_and_bare_resource_ids_are_both_read() {
        assert_eq!(
            id_references("ExtResource(\"2_icon\")"),
            vec![("ExtResource".to_string(), "2_icon".to_string())]
        );
        assert_eq!(
            id_references("ExtResource( 1 )"),
            vec![("ExtResource".to_string(), "1".to_string())]
        );
    }

    #[test]
    fn several_ids_in_one_value_are_all_read() {
        let v = id_references("[SubResource(\"a\"), ExtResource(\"b\")]");
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].1, "b");
    }

    #[test]
    fn an_id_call_inside_a_string_is_not_a_reference() {
        assert!(id_references("\"ExtResource(\\\"9\\\")\"").is_empty());
    }

    #[test]
    fn an_identifier_ending_in_extresource_is_not_a_call() {
        assert!(id_references("MyExtResource(\"1\")").is_empty());
    }

    #[test]
    fn engine_section_list_covers_the_blocks_that_hold_paths() {
        for s in ["application", "autoload", "editor_plugins", "internationalization", "gui"] {
            assert!(ENGINE_SECTIONS.contains(&s), "{} should be an engine section", s);
        }
        // Godot 3 left `[locale]` behind in migrated projects and never reads it.
        assert!(!ENGINE_SECTIONS.contains(&"locale"));
    }
}
