//! `missing-input-action`: a script asks the Input Map for an action the
//! project never defines.
//!
//! The fixture lives in `tests/standalone/inputmap` rather than under
//! `tests/projects`, because CI's action job pins the error and warning totals
//! of `tests/projects` and the recursive test counts its projects. The other
//! shapes are small projects written to a temporary directory.

use std::path::{Path, PathBuf};

use godot_refcheck::checks::{self, Options};
use godot_refcheck::finding::{Finding, Level};
use godot_refcheck::project::Project;

const CHECK: &str = "missing-input-action";

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/standalone/inputmap")
}

fn scan(dir: &Path) -> Vec<Finding> {
    let p = Project::load(dir);
    checks::run(&p, &Options::default()).into_iter().filter(|f| f.check == CHECK).collect()
}

/// A throwaway project: `project.godot` with these `[input]` actions, plus
/// the given files.
fn temp_project(tag: &str, actions: &[&str], files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("refcheck-in-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut godot = String::from("config_version=5\n\n[application]\n\n");
    godot.push_str("config/name=\"t\"\n\n[input]\n\n");
    for a in actions {
        godot.push_str(&format!("{}={{\n\"deadzone\": 0.5,\n\"events\": []\n}}\n", a));
    }
    std::fs::write(dir.join("project.godot"), godot).unwrap();
    for (name, body) in files {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }
    dir
}

#[test]
fn a_typo_in_an_action_name_is_reported_with_the_action_it_meant() {
    let f = scan(&fixture());
    assert_eq!(f.len(), 1, "{:#?}", f);
    assert_eq!(f[0].file, "res://main.gd");
    assert_eq!(f[0].line, 9);
    assert_eq!(f[0].level, Level::Error);
    assert!(f[0].message.contains("\"jmup\""), "{}", f[0].message);
    assert!(f[0].evidence.contains("is_action_pressed()"), "{}", f[0].evidence);
    assert!(f[0].evidence.contains("\"jump\""), "{}", f[0].evidence);
}

#[test]
fn the_fixture_has_nothing_else_to_report() {
    let p = Project::load(&fixture());
    let all = checks::run(&p, &Options::default());
    assert!(all.iter().all(|f| f.check == CHECK), "{:#?}", all);
}

#[test]
fn the_check_can_be_skipped_like_any_other() {
    let p = Project::load(&fixture());
    let o = Options { skip: [CHECK.to_string()].into_iter().collect(), ..Options::default() };
    assert!(checks::run(&p, &o).is_empty());
}

#[test]
fn every_action_of_get_vector_is_judged() {
    let src = "func f():\n\tvar v = Input.get_vector(\"left\", \"right\", \"up\", \"dwon\")\n";
    let dir = temp_project("vector", &["left", "right", "up"], &[("p.gd", src)]);
    let f = scan(&dir);
    assert_eq!(f.len(), 1, "{:#?}", f);
    assert!(f[0].message.contains("\"dwon\""));
    assert_eq!(f[0].line, 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_action_added_in_another_script_is_present() {
    // A settings or remap screen often creates the actions it offers.
    let settings = "func _init():\n\tInputMap.add_action(\"pause\")\n";
    let player = "func _process(_d):\n\tif Input.is_action_just_pressed(\"pause\"): pass\n";
    let dir = temp_project("added", &[], &[("settings.gd", settings), ("player.gd", player)]);
    assert!(scan(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_project_that_adds_actions_under_computed_names_is_not_judged() {
    // Pixelorama builds its shortcut map from data: `InputMap.add_action(action)`.
    let keys = "func load_keys(names):\n\tfor n in names:\n\t\tInputMap.add_action(n)\n";
    let tool = "func _input(e):\n\tif e.is_action_pressed(\"zoom_in\"): pass\n";
    let dir = temp_project("computed", &[], &[("keys.gd", keys), ("tool.gd", tool)]);
    assert!(scan(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_method_the_project_defines_itself_is_not_the_engines() {
    let pad = "class_name Pad\nfunc is_action_pressed(a: String) -> bool:\n\treturn false\n";
    let user = "func f(p: Pad):\n\tif p.is_action_pressed(\"anything\"): pass\n";
    let dir = temp_project("shadowed", &[], &[("pad.gd", pad), ("use.gd", user)]);
    assert!(scan(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn letter_case_matters_and_the_hint_says_which_spelling_exists() {
    // InputMap names are StringNames: "Jump" and "jump" are two actions.
    let src = "func f():\n\tif Input.is_action_pressed(\"Jump\"): pass\n";
    let dir = temp_project("case", &["jump"], &[("p.gd", src)]);
    let f = scan(&dir);
    assert_eq!(f.len(), 1, "{:#?}", f);
    assert!(f[0].evidence.contains("\"jump\""), "{}", f[0].evidence);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hint_is_given_only_when_one_action_is_nearest() {
    let mut src = String::from("func f():\n\tif Input.is_action_pressed(\"fire\"): pass\n");
    src.push_str("\tInput.action_press(\"jupm\")\n");
    let dir = temp_project("near", &["jump", "jumb"], &[("p.gd", src.as_str())]);
    let f = scan(&dir);
    assert_eq!(f.len(), 2, "{:#?}", f);
    // "fire" is far from both; "jupm" is one swap from "jump" and two edits
    // from "jumb", so it gets the nearer one.
    assert!(!f[0].evidence.contains("closest"), "{}", f[0].evidence);
    assert!(f[1].evidence.contains("\"jump\""), "{}", f[1].evidence);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_godot_3_input_section_is_read_the_same_way() {
    let dir = temp_project("legacy", &[], &[]);
    let mut godot = String::from("config_version=4\n\n[input]\n\n");
    godot.push_str("move_left={\n\"deadzone\": 0.5,\n");
    godot.push_str("\"events\": [ Object(InputEventKey,\"scancode\":16777231) ]\n}\n");
    std::fs::write(dir.join("project.godot"), godot).unwrap();
    let src = "func _process(_d):\n\tif Input.is_action_pressed(\"move_left\"): pass\n";
    std::fs::write(dir.join("p.gd"), src).unwrap();
    assert!(scan(&dir).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
