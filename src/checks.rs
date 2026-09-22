use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::finding::{Finding, Level};
use crate::project::{Project, RefKind};

#[derive(Default)]
pub struct Options {
    pub only: Option<BTreeSet<String>>,
    pub skip: BTreeSet<String>,
    pub unused: bool,
}

impl Options {
    fn wants(&self, id: &str) -> bool {
        if self.skip.contains(id) {
            return false;
        }
        match &self.only {
            Some(set) => set.contains(id),
            None => true,
        }
    }
}

pub fn run(p: &Project, o: &Options) -> Vec<Finding> {
    let mut out = Vec::new();
    let dup = duplicate_uids(p);

    if o.wants("duplicate-uid") {
        for (uid, owners) in &dup {
            let mut decls: Vec<_> = p.uid_decls.iter().filter(|d| &d.uid == uid).collect();
            decls.sort_by(|a, b| a.from.cmp(&b.from));
            for d in decls {
                out.push(Finding {
                    project: String::new(),
                    check: "duplicate-uid",
                    level: Level::Error,
                    file: d.from.clone(),
                    line: d.line,
                    message: format!("{} is claimed by {} files", uid, owners.len()),
                    evidence: owners.iter().cloned().collect::<Vec<_>>().join(", "),
                });
            }
        }
    }

    for r in &p.refs {
        let uid_target = r.uid.as_ref().and_then(|u| p.uid_owner.get(u));
        let uid_dup = r.uid.as_ref().map(|u| dup.contains_key(u)).unwrap_or(false);
        match (uid_target, r.path.as_ref()) {
            (Some(t), Some(path)) if t != path && !uid_dup => {
                if o.wants("uid-path-mismatch") {
                    out.push(Finding {
                        project: String::new(),
                        check: "uid-path-mismatch",
                        level: Level::Warning,
                        file: r.from.clone(),
                        line: r.line,
                        message: format!(
                            "{} resolves to {}, but the reference spells the path as {}",
                            r.uid.clone().unwrap_or_default(),
                            t,
                            path
                        ),
                        evidence: origin(r),
                    });
                }
            }
            (Some(_), _) => {}
            (None, Some(path)) => {
                if p.exists(path) {
                    continue;
                }
                if let Some(real) = p.case_variant(path) {
                    if o.wants("case-mismatch") {
                        out.push(Finding {
                            project: String::new(),
                            check: "case-mismatch",
                            level: Level::Error,
                            file: r.from.clone(),
                            line: r.line,
                            message: format!(
                                "{} does not exist; the file on disk is {}",
                                path, real
                            ),
                            evidence: format!(
                                "{} - loads on Windows and macOS, fails on Linux and in exported builds",
                                origin(r)
                            ),
                        });
                    }
                } else if o.wants("missing-resource") {
                    let extra = match &r.uid {
                        Some(u) => format!("; uid {} is also unknown", u),
                        None => String::new(),
                    };
                    out.push(Finding {
                        project: String::new(),
                        check: "missing-resource",
                        level: Level::Error,
                        file: r.from.clone(),
                        line: r.line,
                        message: format!("{} is not in the project{}", path, extra),
                        evidence: origin(r),
                    });
                }
            }
            (None, None) => {
                if o.wants("unknown-uid") {
                    out.push(Finding {
                        project: String::new(),
                        check: "unknown-uid",
                        level: Level::Error,
                        file: r.from.clone(),
                        line: r.line,
                        message: format!(
                            "{} matches no file in the project and no path is given",
                            r.uid.clone().unwrap_or_default()
                        ),
                        evidence: origin(r),
                    });
                }
            }
        }
    }

    if o.wants("undeclared-id") {
        for u in &p.id_uses {
            let key = (u.from.clone(), u.kind.clone(), u.id.clone());
            if !p.id_decls.contains(&key) {
                out.push(Finding {
                    project: String::new(),
                    check: "undeclared-id",
                    level: Level::Error,
                    file: u.from.clone(),
                    line: u.line,
                    message: format!("{}(\"{}\") has no matching declaration", u.kind, u.id),
                    evidence: format!("the file declares no {} with id {}", u.kind, u.id),
                });
            }
        }
    }

    if o.wants("stale-import") {
        for (file, line, source) in &p.stale_imports {
            out.push(Finding {
                project: String::new(),
                check: "stale-import",
                level: Level::Warning,
                file: file.clone(),
                line: *line,
                message: format!("import metadata left over from {}, which is gone", source),
                evidence: "delete the .import file, or restore the asset".into(),
            });
        }
    }

    if o.unused && o.wants("unused-asset") {
        out.extend(unused_assets(p));
    }

    crate::finding::sort(&mut out);
    out
}

fn origin(r: &crate::project::Reference) -> String {
    if r.detail.is_empty() {
        r.kind.label().to_string()
    } else {
        format!("{} ({})", r.kind.label(), r.detail)
    }
}

fn duplicate_uids(p: &Project) -> BTreeMap<String, BTreeSet<String>> {
    let mut by_uid: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for d in &p.uid_decls {
        by_uid.entry(d.uid.clone()).or_default().insert(d.owner.clone());
    }
    by_uid.into_iter().filter(|(_, owners)| owners.len() > 1).collect()
}

/// Extensions that are meaningful to look at for the advisory unused scan.
/// Scripts are deliberately excluded: a script can be reached through a
/// `class_name` with no res:// reference anywhere.
const UNUSED_EXTS: &[&str] = &["tscn", "tres", "theme", "material"];

fn unused_assets(p: &Project) -> Vec<Finding> {
    let mut referenced: HashSet<String> = HashSet::new();
    for r in &p.refs {
        if let Some(t) = r.uid.as_ref().and_then(|u| p.uid_owner.get(u)) {
            referenced.insert(t.clone());
        }
        if let Some(path) = &r.path {
            referenced.insert(path.clone());
            if let Some(real) = p.case_variant(path) {
                referenced.insert(real.clone());
            }
        }
    }
    // An asset whose imported product is referenced (a .csv that becomes a
    // .translation, for instance) is itself in use.
    for (source, products) in &p.import_products {
        if products.iter().any(|x| referenced.contains(x)) {
            referenced.insert(source.clone());
        }
    }
    // Anything named in a script string literal is treated as reachable.
    let mut literal_names: HashSet<String> = HashSet::new();
    for lits in p.literals.values() {
        for l in lits {
            literal_names.insert(l.clone());
            if let Some(name) = l.rsplit('/').next() {
                literal_names.insert(name.to_string());
            }
        }
    }

    let mut out = Vec::new();
    for f in &p.files {
        if f.starts_with("res://addons/") || f.contains("/addons/") {
            continue;
        }
        if referenced.contains(f) {
            continue;
        }
        let name = f.rsplit('/').next().unwrap_or("");
        let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        let has_import = p.files.contains(&format!("{}.import", f));
        if !has_import && !UNUSED_EXTS.contains(&ext.as_str()) {
            continue;
        }
        if literal_names.contains(name) || literal_names.contains(f) {
            continue;
        }
        out.push(Finding {
            project: String::new(),
            check: "unused-asset",
            level: Level::Info,
            file: f.clone(),
            line: 1,
            message: "nothing in the project references this file".into(),
            evidence: "advisory: assets loaded through a computed path cannot be seen statically"
                .into(),
        });
    }
    out
}

pub fn ref_count(p: &Project) -> usize {
    p.refs.len()
}

pub fn kind_counts(p: &Project) -> BTreeMap<&'static str, usize> {
    let mut m: BTreeMap<&'static str, usize> = BTreeMap::new();
    for r in &p.refs {
        *m.entry(r.kind.label()).or_insert(0) += 1;
    }
    let _ = RefKind::ExtResource;
    m
}
