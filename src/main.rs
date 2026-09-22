use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use godot_refcheck::checks::{self, Options};
use godot_refcheck::finding::{Finding, Level, CHECKS};
use godot_refcheck::project::{find_project_root, Project};
use godot_refcheck::report;

const USAGE: &str = "\
godot-refcheck - find broken resource references in Godot projects

USAGE:
    godot-refcheck [PATH] [OPTIONS]

    PATH   a project directory, a project.godot file, or (with --recursive)
           a directory containing several projects. Defaults to the current
           directory.

OPTIONS:
    --recursive          scan every project.godot found under PATH
    --unused             also report assets nothing appears to reference (note level)
    --only <ids>         run only these checks (comma separated)
    --skip <ids>         skip these checks (comma separated)
    --fail-on <level>    exit 1 at this level or above: error, warning, info, never
                         (default: error)
    --json               print machine-readable JSON instead of text
    --sarif <file>       also write a SARIF 2.1.0 report for GitHub code scanning
    --quiet              print nothing but the summary line
    --list-checks        list the checks and exit
    -h, --help           print this help
    -V, --version        print the version

EXIT CODES:
    0  no finding at or above --fail-on
    1  at least one finding at or above --fail-on
    2  the project could not be read
";

struct Args {
    path: PathBuf,
    recursive: bool,
    json: bool,
    quiet: bool,
    sarif: Option<PathBuf>,
    fail_on: Option<Level>,
    opts: Options,
}

fn parse_args(argv: Vec<String>) -> Result<Args, String> {
    let mut a = Args {
        path: PathBuf::from("."),
        recursive: false,
        json: false,
        quiet: false,
        sarif: None,
        fail_on: Some(Level::Error),
        opts: Options::default(),
    };
    let mut seen_path = false;
    let mut i = 0usize;
    while i < argv.len() {
        let arg = argv[i].clone();
        let mut next = |name: &str| -> Result<String, String> {
            i += 1;
            argv.get(i).cloned().ok_or_else(|| format!("{} needs a value", name))
        };
        match arg.as_str() {
            "-h" | "--help" => return Err("@help".into()),
            "-V" | "--version" => return Err("@version".into()),
            "--list-checks" => return Err("@checks".into()),
            "--recursive" | "-r" => a.recursive = true,
            "--unused" => a.opts.unused = true,
            "--json" => a.json = true,
            "--quiet" | "-q" => a.quiet = true,
            "--sarif" => a.sarif = Some(PathBuf::from(next("--sarif")?)),
            "--only" => {
                let v = next("--only")?;
                a.opts.only = Some(split_ids(&v));
                a.opts.unused = a.opts.unused || v.split(',').any(|s| s.trim() == "unused-asset");
            }
            "--skip" => a.opts.skip = split_ids(&next("--skip")?),
            "--fail-on" => {
                let v = next("--fail-on")?;
                a.fail_on = if v.eq_ignore_ascii_case("never") {
                    None
                } else {
                    Some(Level::parse(&v).ok_or_else(|| format!("unknown level: {}", v))?)
                };
            }
            other if other.starts_with('-') => return Err(format!("unknown option: {}", other)),
            other => {
                if seen_path {
                    return Err("only one PATH is accepted".into());
                }
                a.path = PathBuf::from(other);
                seen_path = true;
            }
        }
        i += 1;
    }
    let known: BTreeSet<&str> = CHECKS.iter().map(|c| c.id).collect();
    for set in [a.opts.only.as_ref(), Some(&a.opts.skip)].into_iter().flatten() {
        for id in set {
            if !known.contains(id.as_str()) {
                return Err(format!("unknown check: {}", id));
            }
        }
    }
    Ok(a)
}

fn relative_label(base: &Path, root: &Path) -> String {
    match root.strip_prefix(base) {
        Ok(r) if r.as_os_str().is_empty() => String::new(),
        Ok(r) => r.display().to_string().replace('\\', "/"),
        Err(_) => root.display().to_string().replace('\\', "/"),
    }
}

fn split_ids(v: &str) -> BTreeSet<String> {
    v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

fn find_projects(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.join("project.godot").is_file() {
        out.push(dir.to_path_buf());
        return;
    }
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut subs: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            !n.starts_with('.') && n != "node_modules"
        })
        .collect();
    subs.sort();
    for s in subs {
        find_projects(&s, out);
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(argv) {
        Ok(a) => a,
        Err(e) if e == "@help" => {
            print!("{}", USAGE);
            return ExitCode::from(0);
        }
        Err(e) if e == "@version" => {
            println!("godot-refcheck {}", report::VERSION);
            return ExitCode::from(0);
        }
        Err(e) if e == "@checks" => {
            for c in CHECKS {
                println!("{:<18} {:<8} {}", c.id, c.level.as_str(), c.summary);
            }
            return ExitCode::from(0);
        }
        Err(e) => {
            eprintln!("godot-refcheck: {}\n\n{}", e, USAGE);
            return ExitCode::from(2);
        }
    };

    let mut roots: Vec<PathBuf> = Vec::new();
    if args.recursive {
        find_projects(&args.path, &mut roots);
        if roots.is_empty() {
            eprintln!("godot-refcheck: no project.godot found under {}", args.path.display());
            return ExitCode::from(2);
        }
    } else {
        match find_project_root(&args.path) {
            Some(r) => roots.push(r),
            None => {
                eprintln!(
                    "godot-refcheck: {} is not inside a Godot project (no project.godot found)",
                    args.path.display()
                );
                return ExitCode::from(2);
            }
        }
    }

    let mut all: Vec<Finding> = Vec::new();
    let mut scanned = 0usize;
    let mut refs = 0usize;
    let mut chunks: Vec<String> = Vec::new();

    for root in &roots {
        let p = Project::load(root);
        let mut f = checks::run(&p, &args.opts);
        let label = relative_label(&args.path, root);
        for item in f.iter_mut() {
            item.project = label.clone();
        }
        scanned += p.files.len();
        refs += checks::ref_count(&p);
        if !args.quiet && !args.json && (roots.len() == 1 || !f.is_empty()) {
            chunks.push(report::text(
                &f,
                p.files.len(),
                checks::ref_count(&p),
                &root.display().to_string(),
            ));
        }
        all.extend(f);
    }

    godot_refcheck::finding::sort(&mut all);

    if args.json {
        print!("{}", report::json(&all, scanned, refs, &args.path.display().to_string()));
    } else if args.quiet {
        println!("{}", report::summary(&all, roots.len(), scanned, refs));
    } else {
        for c in &chunks {
            print!("{}", c);
        }
        if roots.len() > 1 {
            println!("\n{}", report::summary(&all, roots.len(), scanned, refs));
        }
    }

    if let Some(path) = &args.sarif {
        if let Err(e) = std::fs::write(path, report::sarif(&all)) {
            eprintln!("godot-refcheck: cannot write {}: {}", path.display(), e);
            return ExitCode::from(2);
        }
    }

    match args.fail_on {
        Some(level) if all.iter().any(|f| f.level >= level) => ExitCode::from(1),
        _ => ExitCode::from(0),
    }
}
