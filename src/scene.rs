//! The node tree of a scene, resolved through instanced and inherited scenes.
//!
//! A `.tscn` only lists the nodes it owns. A node coming from an instanced
//! scene, or from the scene it inherits from, is not written down anywhere in
//! the file, so a connection that points at one looks broken unless the other
//! scene is opened too. This module opens them.

use std::collections::{BTreeMap, BTreeSet};

use crate::project::Project;

const MAX_DEPTH: usize = 12;

#[derive(Debug, Clone, Default)]
pub struct Tree {
    /// Every node path relative to the scene root; the root itself is ".".
    pub paths: BTreeSet<String>,
    /// Subtrees whose contents are unknown, so nothing under them can be judged.
    pub opaque: Vec<String>,
    /// The scene could not be read at all.
    pub unresolved: bool,
}

impl Tree {
    fn blind() -> Tree {
        Tree { unresolved: true, ..Default::default() }
    }

    /// Is `path` known to be absent? Unknown subtrees answer `false`.
    pub fn is_missing(&self, path: &str) -> bool {
        if self.unresolved || self.paths.contains(path) {
            return false;
        }
        !self.opaque.iter().any(|o| path == o || path.starts_with(&format!("{}/", o)))
    }
}

fn join(parent: &str, name: &str) -> String {
    if parent == "." {
        name.to_string()
    } else {
        format!("{}/{}", parent, name)
    }
}

pub struct Resolver<'a> {
    project: &'a Project,
    cache: BTreeMap<String, Tree>,
}

impl<'a> Resolver<'a> {
    pub fn new(project: &'a Project) -> Resolver<'a> {
        Resolver { project, cache: BTreeMap::new() }
    }

    pub fn tree(&mut self, res: &str) -> Tree {
        if let Some(t) = self.cache.get(res) {
            return t.clone();
        }
        let t = self.build(res, 0, &mut Vec::new());
        self.cache.insert(res.to_string(), t.clone());
        t
    }

    fn build(&mut self, res: &str, depth: usize, stack: &mut Vec<String>) -> Tree {
        if depth > MAX_DEPTH || stack.iter().any(|s| s == res) {
            return Tree::blind();
        }
        let scene = match self.project.scenes.get(res) {
            Some(s) => s.clone(),
            None => return Tree::blind(),
        };
        stack.push(res.to_string());

        let mut tree = Tree::default();
        tree.paths.insert(".".to_string());

        for node in &scene.nodes {
            let path = match &node.parent {
                None => ".".to_string(),
                Some(p) if p == "." => node.name.clone(),
                Some(p) => join(p, &node.name),
            };
            tree.paths.insert(path.clone());

            if node.placeholder {
                tree.opaque.push(path.clone());
                continue;
            }
            let target = match &node.instance {
                Some(id) => scene.target_of(id, self.project),
                None => None,
            };
            if node.instance.is_some() {
                match target {
                    Some(t) => {
                        let sub = self.build(&t, depth + 1, stack);
                        if sub.unresolved {
                            tree.opaque.push(path.clone());
                        } else {
                            for p in &sub.paths {
                                if p != "." {
                                    tree.paths.insert(join(&path, p));
                                }
                            }
                            for o in &sub.opaque {
                                tree.opaque.push(join(&path, o));
                            }
                        }
                    }
                    None => tree.opaque.push(path.clone()),
                }
            }
        }

        stack.pop();
        tree
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodeDecl {
    pub name: String,
    /// `None` only for the root node.
    pub parent: Option<String>,
    pub instance: Option<String>,
    pub placeholder: bool,
    pub line: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ConnDecl {
    pub signal: String,
    pub from: String,
    pub to: String,
    pub method: String,
    pub line: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ExtTarget {
    pub path: Option<String>,
    pub uid: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SceneFile {
    pub nodes: Vec<NodeDecl>,
    pub connections: Vec<ConnDecl>,
    pub ext: BTreeMap<String, ExtTarget>,
    /// node path (relative to the scene root, root is ".") -> the
    /// `ExtResource` id of the script attached to it. Needed to answer "which
    /// scene is this script's `$Head/Body` written against", which is the
    /// only way a path inside a script can be resolved at all.
    pub scripts: BTreeMap<String, String>,
}

impl SceneFile {
    pub fn target_of(&self, id: &str, project: &Project) -> Option<String> {
        let e = self.ext.get(id)?;
        if let Some(uid) = &e.uid {
            if let Some(owner) = project.uid_owner.get(uid) {
                return Some(owner.clone());
            }
        }
        e.path.clone()
    }
}

/// A node path that cannot be judged: unique names and relative steps are
/// resolved by the engine at run time, not by reading the file.
pub fn judgeable(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('%')
        && !path.split('/').any(|s| s == ".." || s.is_empty())
        && !path.starts_with("/root")
}
