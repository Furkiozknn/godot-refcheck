//! End-to-end checks against small Godot projects kept under `tests/projects`.
//! Every expectation here comes from a shape seen in a real repository; the
//! comments name the repository where the shape was found.

use std::collections::BTreeSet;
use std::path::PathBuf;

use godot_refcheck::checks::{self, Options};
use godot_refcheck::finding::{Finding, Level};
use godot_refcheck::project::Project;

fn project_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/projects").join(name)
}

fn scan(name: &str) -> Vec<Finding> {
    let p = Project::load(&project_dir(name));
    checks::run(&p, &Options::default())
}

fn scan_with(name: &str, o: Options) -> Vec<Finding> {
    let p = Project::load(&project_dir(name));
    checks::run(&p, &o)
}

fn scan_unused(name: &str) -> Vec<Finding> {
    scan_with(name, Options { unused: true, ..Options::default() })
}

fn of(findings: &[Finding], check: &str) -> Vec<Finding> {
    findings.iter().filter(|f| f.check == check).cloned().collect()
}

fn files(findings: &[Finding], check: &str) -> BTreeSet<String> {
    of(findings, check).into_iter().map(|f| f.file).collect()
}

#[test]
fn a_healthy_project_produces_nothing() {
    let f = scan("clean");
    assert!(f.is_empty(), "unexpected findings: {:#?}", f);
}

#[test]
fn a_healthy_project_produces_nothing_with_the_unused_scan_on() {
    let f = scan_unused("clean");
    assert!(f.is_empty(), "unexpected findings: {:#?}", f);
}

/// A `.csv` becomes `text.en.translation` at import time and the generated file
/// is normally git-ignored. Godot regenerates it, so a fresh clone is not broken
/// and the csv itself is in use.
#[test]
fn generated_translations_count_as_present_and_their_source_as_used() {
    let p = Project::load(&project_dir("clean"));
    assert!(p.exists("res://translations/text.en.translation"));
    assert!(!p.files.contains("res://translations/text.en.translation"));
    let f = scan_unused("clean");
    assert!(of(&f, "unused-asset").is_empty());
}

#[test]
fn every_error_class_is_found_in_the_broken_project() {
    let f = scan("broken");
    for check in [
        "missing-resource",
        "case-mismatch",
        "unknown-uid",
        "duplicate-uid",
        "undeclared-id",
        "uid-path-mismatch",
        "stale-import",
    ] {
        assert!(!of(&f, check).is_empty(), "{} was not reported", check);
    }
}

#[test]
fn a_missing_ext_resource_is_reported_on_its_own_line() {
    let f = of(&scan("broken"), "missing-resource");
    let scene = f.iter().find(|x| x.file == "res://main.tscn").expect("scene finding");
    assert_eq!(scene.line, 4);
    assert!(scene.message.contains("res://art/absent.png"));
}

#[test]
fn a_broken_preload_is_reported_against_the_script() {
    let f = of(&scan("broken"), "missing-resource");
    let script = f.iter().find(|x| x.file == "res://main.gd").expect("script finding");
    assert_eq!(script.line, 3);
}

#[test]
fn a_missing_autoload_is_reported_against_project_godot() {
    let f = of(&scan("broken"), "missing-resource");
    let setting = f.iter().find(|x| x.file == "res://project.godot").expect("setting finding");
    assert!(setting.evidence.contains("autoload/Gone"));
}

/// The file on disk is `art/present.png`; the scene asks for `Art/Present.PNG`.
/// Windows and macOS open it, Linux and exported builds do not.
#[test]
fn a_case_only_difference_is_its_own_finding() {
    let f = of(&scan("broken"), "case-mismatch");
    assert_eq!(f.len(), 1);
    assert!(f[0].message.contains("res://art/present.png"));
    assert_eq!(f[0].level, Level::Error);
}

#[test]
fn a_duplicate_uid_is_reported_against_both_owners() {
    assert_eq!(
        files(&scan("broken"), "duplicate-uid"),
        ["res://main.tscn", "res://twin.tscn"].iter().map(|s| s.to_string()).collect()
    );
}

#[test]
fn a_uid_with_no_path_and_no_owner_is_an_unknown_uid() {
    let f = of(&scan("broken"), "unknown-uid");
    assert_eq!(f.len(), 1);
    assert!(f[0].message.contains("uid://cnobodyhasthisu"));
}

#[test]
fn an_id_used_but_never_declared_is_reported() {
    let f = of(&scan("broken"), "undeclared-id");
    assert_eq!(f.len(), 1);
    assert!(f[0].message.contains("99"));
}

/// When the uid still resolves, the engine loads the uid target and silently
/// ignores the stale path. The project works, so this is a warning, not an error.
#[test]
fn a_uid_that_disagrees_with_its_path_is_a_warning() {
    let f = of(&scan("broken"), "uid-path-mismatch");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].level, Level::Warning);
    assert!(f[0].message.contains("res://art/present.png"));
    assert!(f[0].message.contains("res://art/old_name.png"));
}

#[test]
fn import_metadata_without_its_asset_is_a_warning() {
    let f = of(&scan("broken"), "stale-import");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].file, "res://art/deleted.png.import");
    assert_eq!(f[0].level, Level::Warning);
}

/// Godot 3 scenes use `format=2`, unquoted ids and `ExtResource( 1 )`.
#[test]
fn a_godot_3_project_is_understood() {
    let f = scan("legacy3");
    assert!(f.is_empty(), "unexpected findings: {:#?}", f);
    let p = Project::load(&project_dir("legacy3"));
    assert_eq!(p.refs.len(), 5);
}

/// Everything in this project looks broken to a naive reader and is not:
/// a dead Godot 3 `[locale]` block, locale-suffixed remaps, run-time paths,
/// an editor-written `[replication]` block and a sub-resource path.
#[test]
fn shapes_that_only_look_broken_produce_no_error() {
    let f = scan("tricky");
    assert!(f.is_empty(), "unexpected findings: {:#?}", f);
}

#[test]
fn the_unused_scan_finds_the_one_asset_nothing_points_at() {
    let f = of(&scan_unused("tricky"), "unused-asset");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].file, "res://dead/unused.tres");
    assert_eq!(f[0].level, Level::Info);
}

#[test]
fn the_unused_scan_stays_off_unless_it_is_asked_for() {
    assert!(of(&scan("tricky"), "unused-asset").is_empty());
}

#[test]
fn only_runs_a_single_check() {
    let mut only = BTreeSet::new();
    only.insert("duplicate-uid".to_string());
    let f = scan_with("broken", Options { only: Some(only), ..Options::default() });
    assert!(f.iter().all(|x| x.check == "duplicate-uid"));
    assert_eq!(f.len(), 2);
}

#[test]
fn skip_removes_a_check_and_leaves_the_rest() {
    let mut skip = BTreeSet::new();
    skip.insert("duplicate-uid".to_string());
    let f = scan_with("broken", Options { skip, ..Options::default() });
    assert!(of(&f, "duplicate-uid").is_empty());
    assert!(!of(&f, "missing-resource").is_empty());
}

#[test]
fn findings_are_ordered_with_errors_first() {
    let f = scan("broken");
    let levels: Vec<Level> = f.iter().map(|x| x.level).collect();
    let mut sorted = levels.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(levels, sorted);
}

#[test]
fn a_plugin_cfg_script_is_resolved_next_to_the_cfg() {
    let p = Project::load(&project_dir("clean"));
    assert!(p.refs.iter().any(|r| r.path.as_deref() == Some("res://addons/demo/plugin.gd")));
}

#[test]
fn a_shader_include_is_resolved_next_to_the_shader() {
    let p = Project::load(&project_dir("clean"));
    assert!(p.refs.iter().any(|r| r.path.as_deref() == Some("res://shaders/base.gdshaderinc")));
}

#[test]
fn the_godot_cache_directory_is_never_scanned() {
    let p = Project::load(&project_dir("clean"));
    assert!(p.files.iter().all(|f| !f.starts_with("res://.godot/")));
}

// --- the scene graph -------------------------------------------------------

/// A connection into an instanced scene, into a scene this one inherits from,
/// or under a placeholder instance points at a node no line of this file
/// mentions, and all three are perfectly healthy.
#[test]
fn connections_through_instances_and_inheritance_are_not_broken() {
    let f = of(&scan("conn"), "broken-connection");
    let lines: Vec<(String, usize)> = f.iter().map(|x| (x.file.clone(), x.line)).collect();
    assert_eq!(
        lines,
        vec![("res://heir.tscn".to_string(), 11), ("res://level.tscn".to_string(), 25)]
    );
}

#[test]
fn a_connection_to_a_node_that_is_gone_is_reported_once_per_end() {
    let f = of(&scan("conn"), "broken-connection");
    assert!(f.iter().any(|x| x.message.contains("Panel/Gone")));
    assert!(f.iter().all(|x| !x.message.contains("Panel/Inner")));
}

#[test]
fn a_placeholder_subtree_is_never_judged() {
    let f = of(&scan("conn"), "broken-connection");
    assert!(f.iter().all(|x| !x.message.contains("Later/")));
}

#[test]
fn the_node_tree_reaches_into_an_inherited_base() {
    let p = Project::load(&project_dir("conn"));
    let mut r = godot_refcheck::scene::Resolver::new(&p);
    let t = r.tree("res://level.tscn");
    for path in
        ["Button", "Panel", "Panel/Inner", "Panel/Inner/Deep", "Heir/FromBase", "Heir/Added"]
    {
        assert!(t.paths.contains(path), "{} should be in the tree", path);
    }
    assert!(t.is_missing("Panel/Gone"));
    assert!(!t.is_missing("Later/Anything"));
}

#[test]
fn a_scene_whose_instance_cannot_be_read_is_not_judged() {
    let p = Project::load(&project_dir("broken"));
    let mut r = godot_refcheck::scene::Resolver::new(&p);
    let t = r.tree("res://does_not_exist.tscn");
    assert!(t.unresolved);
    assert!(!t.is_missing("Anything"));
}

// --- scripts ---------------------------------------------------------------

#[test]
fn two_scripts_with_the_same_class_name_are_both_reported() {
    let f = of(&scan("scripts"), "duplicate-class-name");
    assert_eq!(f.len(), 2);
    assert!(f.iter().all(|x| x.message.contains("Hero")));
}

#[test]
fn a_super_class_path_that_does_not_exist_is_a_missing_resource() {
    let f = of(&scan("scripts"), "missing-resource");
    assert!(f.iter().any(|x| x.message.contains("res://no_such_base.gd")));
}

#[test]
fn an_icon_annotation_is_a_reference() {
    let p = Project::load(&project_dir("scripts"));
    assert!(p.refs.iter().any(|r| r.path.as_deref() == Some("res://missing_icon.svg")));
}

// --- repairs ---------------------------------------------------------------

#[test]
fn a_moved_file_is_offered_in_the_finding_text() {
    let f = of(&scan("moved"), "missing-resource");
    assert!(f.iter().any(|x| x.evidence.contains("res://art/tiles/wall.png")));
}

#[test]
fn every_repair_in_the_moved_project_can_be_proved() {
    let p = Project::load(&project_dir("moved"));
    let r = godot_refcheck::checks::repairs(&p, &Options::default());
    assert_eq!(r.len(), 3);
    assert!(r.iter().any(|x| x.new == "res://player.tscn"));
    assert!(r.iter().any(|x| x.new == "res://art/tiles/wall.png"));
    assert!(r.iter().any(|x| x.new == "res://ui/Icon.png"));
}

/// Two files with the same name give no single answer, so nothing is proposed.
#[test]
fn an_ambiguous_name_produces_no_repair() {
    let p = Project::load(&project_dir("tricky"));
    let r = godot_refcheck::checks::repairs(&p, &Options::default());
    assert!(r.is_empty(), "{:?}", r);
}

#[test]
fn a_healthy_project_is_never_rewritten() {
    for name in ["clean", "legacy3", "conn"] {
        let p = Project::load(&project_dir(name));
        let r = godot_refcheck::checks::repairs(&p, &Options::default());
        assert!(r.is_empty(), "{} would be rewritten: {:?}", name, r);
    }
}

// ---------------------------------------------------------------------------
// missing-node-path: `$Head/Body` inside a script
//
// The shape comes from this ecosystem's own games: four shipped Godot projects
// with **no** `[connection]` blocks at all and 145 `.connect(` call sites in
// GDScript. The resolver that judges a connection's `to=` was pointed at a
// file section those projects never write.
// ---------------------------------------------------------------------------

#[test]
fn a_script_path_that_the_scene_does_not_contain_is_reported() {
    let f = of(&scan("nodepath"), "missing-node-path");
    let paths: BTreeSet<String> =
        f.iter().map(|x| x.message.split('"').nth(1).unwrap_or("").to_string()).collect();
    assert_eq!(paths, ["Panel/Gitti", "Yok"].iter().map(|s| s.to_string()).collect());
}

#[test]
fn it_is_reported_against_the_script_not_the_scene() {
    // The line a human has to change is in the .gd file.
    let f = of(&scan("nodepath"), "missing-node-path");
    assert!(f.iter().all(|x| x.file.ends_with("level.gd")), "{:#?}", f);
    assert!(f.iter().all(|x| x.line > 1), "{:#?}", f);
}

#[test]
fn a_path_that_resolves_through_an_instanced_scene_is_silent() {
    // `$Panel/Deep` reaches into inner.tscn, which level.tscn instances.
    let f = of(&scan("nodepath"), "missing-node-path");
    assert!(!f.iter().any(|x| x.message.contains("Panel/Deep")), "{:#?}", f);
}

#[test]
fn nothing_under_a_placeholder_is_judged() {
    let f = of(&scan("nodepath"), "missing-node-path");
    assert!(!f.iter().any(|x| x.message.contains("Later")), "{:#?}", f);
}

#[test]
fn a_node_some_script_names_at_run_time_is_not_called_missing() {
    // `n.name = "Runtime"; add_child(n)` puts a node in no .tscn at all. A
    // shipped game does exactly this (`alet.name = "Aletler"`), and calling
    // it missing was this check's only false positive across five real
    // projects.
    let f = of(&scan("nodepath"), "missing-node-path");
    assert!(!f.iter().any(|x| x.message.contains("Runtime")), "{:#?}", f);
}

#[test]
fn a_child_that_is_an_instanced_scene_carries_that_scenes_root_name() {
    // `add_child(load("res://bubble.tscn").instantiate())` names the child
    // "Bubble" - bubble.tscn's root. Which scene a call site instantiates is
    // usually not knowable statically (a variable here, a PackedScene
    // parameter in godot-demo-projects, an exported array in dialogic), so a
    // missing segment spelled like SOME scene's root buys silence.
    let f = of(&scan("nodepath"), "missing-node-path");
    assert!(!f.iter().any(|x| x.message.contains("Bubble")), "{:#?}", f);
}

#[test]
fn a_path_the_script_guards_with_has_node_is_left_alone() {
    // `if has_node("Optional"): get_node("Optional")` is the author stating
    // the node is optional. The engine returns null and the branch is not
    // entered; reporting it repeats what the code already says.
    let f = of(&scan("nodepath"), "missing-node-path");
    assert!(!f.iter().any(|x| x.message.contains("Optional")), "{:#?}", f);
}

#[test]
fn the_same_path_written_twice_on_one_line_is_one_finding() {
    // `$A/B.x = -$A/B.y` is one claim to judge. Counting the offsets rather
    // than the (line, path) pairs printed material-maker's paint.gd:910
    // twice, word for word.
    let src = "extends Node\nfunc f():\n\t$Yok.a = -$Yok.b\n";
    let claims = godot_refcheck::parse::gdscript::node_paths(src);
    let lines: std::collections::BTreeSet<(usize, String)> =
        claims.iter().map(|(_, p)| (0usize, p.clone())).collect();
    assert_eq!(claims.len(), 2, "both offsets are found: {:#?}", claims);
    assert_eq!(lines.len(), 1, "but they are one claim: {:#?}", lines);
}

#[test]
fn a_path_asked_of_another_node_is_not_this_scenes_business() {
    // `other.get_node("Nope")` is written against whatever `other` is. On one
    // shipped game, ignoring the receiver produced 132 findings - every one a
    // test harness reaching into a scene it had just instantiated.
    let f = of(&scan("nodepath"), "missing-node-path");
    assert!(!f.iter().any(|x| x.message.contains("Nope")), "{:#?}", f);
}

#[test]
fn a_computed_or_unique_path_is_left_alone() {
    let f = of(&scan("nodepath"), "missing-node-path");
    assert!(!f.iter().any(|x| x.message.contains("Benzersiz")), "{:#?}", f);
    assert!(!f.iter().any(|x| x.message.contains("Panel/\" +")), "{:#?}", f);
}

#[test]
fn a_script_on_a_child_resolves_from_that_child() {
    // child.gd sits on "Kid" and says `$Sprite`, meaning "Kid/Sprite".
    // Resolving from the scene root would invent a finding.
    let f = of(&scan("nodepath"), "missing-node-path");
    assert!(!f.iter().any(|x| x.file.ends_with("child.gd")), "{:#?}", f);
}

#[test]
fn a_script_no_scene_attaches_is_not_judged() {
    // An autoload, a RefCounted helper, a script attached from code: none of
    // them has a tree to be judged against.
    let f = of(&scan("scripts"), "missing-node-path");
    assert!(f.is_empty(), "{:#?}", f);
}

#[test]
fn the_check_can_be_skipped_like_any_other() {
    let mut skip = BTreeSet::new();
    skip.insert("missing-node-path".to_string());
    let f = scan_with("nodepath", Options { skip, ..Options::default() });
    assert!(of(&f, "missing-node-path").is_empty());
}
