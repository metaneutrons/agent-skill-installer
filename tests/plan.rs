// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! M2 of docs/plans/agent-skill-installer.md: the planner, against whole
//! machines described in temporary directories.

use std::fs;
use std::path::{Path, PathBuf};

use agent_skill_installer::{Catalog, CatalogError, Environment, Location, SkillDir, Tool, plan};

/// The home directory of each built-in tool, in catalog order.
const HOMES: [(&str, &str); 6] = [
    ("claude-code", ".claude"),
    ("codex", ".codex"),
    ("gemini-cli", ".gemini"),
    ("copilot", ".copilot"),
    ("cursor", ".cursor"),
    ("opencode", ".config/opencode"),
];

fn machine(tools: &[&str]) -> tempfile::TempDir {
    let home = tempfile::tempdir().expect("temp home");
    for (id, dir) in HOMES {
        if tools.contains(&id) {
            fs::create_dir_all(home.path().join(dir)).expect("tool home");
        }
    }
    home
}

/// A canonical path as the crate reports places: on Windows without the
/// verbatim `\\\\?\\` prefix, as people and tools write it.
fn real(path: &Path) -> PathBuf {
    let canonical = fs::canonicalize(path).expect("canonical");
    if cfg!(windows)
        && let Some(rest) = canonical.to_string_lossy().strip_prefix(r"\\?\")
        && rest.as_bytes().get(1) == Some(&b':')
    {
        return PathBuf::from(rest);
    }
    canonical
}

/// The catalog directories of each target, which is what the selection rule
/// decides.
fn chosen(home: &Path) -> (Vec<Vec<String>>, Vec<String>) {
    let plan = plan(&Catalog::builtin(), &Environment::new(home)).expect("plan");
    (
        plan.targets
            .iter()
            .map(|target| target.dirs.clone())
            .collect(),
        plan.duplicated,
    )
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(ToString::to_string).collect()
}

/// M2-A1: a tool counts as installed exactly when its home exists, for every
/// one of the 64 combinations of the six built-in tools.
#[test]
fn every_combination_of_tool_homes_is_detected_as_it_is() {
    for mask in 0u32..64 {
        let present: Vec<&str> = HOMES
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, (id, _))| *id)
            .collect();
        let home = machine(&present);
        let plan = plan(&Catalog::builtin(), &Environment::new(home.path())).expect("plan");
        assert_eq!(plan.installed, ids(&present), "mask {mask:06b}");
        // Nothing installed, nothing planned; something installed, something
        // planned, because every built-in tool has a directory it may create.
        assert_eq!(
            plan.targets.is_empty(),
            present.is_empty(),
            "mask {mask:06b}"
        );
        assert!(
            plan.unserved.is_empty(),
            "mask {mask:06b}: {:?}",
            plan.unserved
        );
    }
}

/// M2-A1, the other half of D2: a directory is only created inside a tool home
/// that exists, and never for a tool that is not installed.
#[test]
fn no_tool_home_is_ever_created_for_a_tool_that_is_absent() {
    let home = machine(&["opencode"]);
    let plan = plan(&Catalog::builtin(), &Environment::new(home.path())).expect("plan");
    // OpenCode reads ~/.claude/skills too, but ~/.claude does not exist, so
    // that directory is not a candidate.
    assert_eq!(plan.targets.len(), 1);
    assert_eq!(plan.targets[0].dirs, ids(&["agents"]));
    assert!(plan.targets[0].create);
    assert!(
        !home.path().join(".claude").exists(),
        "planning must not write"
    );
}

/// M2-A2: the maintainer's machine from the plan's baseline, rebuilt.
///
/// `~/.claude/skills` is a git working tree, `~/.codex/skills` holds symlinks
/// into it, and `~/.gemini/skills` is itself a symlink to it. The two names
/// for the same place become one target; Codex, which reads only
/// `~/.agents/skills`, needs a second; and Gemini CLI, which reads both, is
/// named as the tool that will see the skill twice.
#[cfg(unix)]
#[test]
fn the_maintainers_layout_resolves_to_its_physical_places() {
    use std::os::unix::fs::symlink;

    let home = machine(&["claude-code", "codex", "gemini-cli"]);
    let h = home.path();
    let claude_skills = h.join(".claude/skills");
    fs::create_dir_all(claude_skills.join("amazon")).expect("a skill");
    fs::create_dir_all(claude_skills.join(".git")).expect("a repository");
    fs::create_dir_all(h.join(".codex/skills")).expect("codex skills");
    symlink(claude_skills.join("amazon"), h.join(".codex/skills/amazon")).expect("link");
    symlink(&claude_skills, h.join(".gemini/skills")).expect("root link");

    let plan = plan(&Catalog::builtin(), &Environment::new(h)).expect("plan");

    assert_eq!(plan.installed, ids(&["claude-code", "codex", "gemini-cli"]));
    assert_eq!(plan.targets.len(), 2, "{:#?}", plan.targets);

    let shared = &plan.targets[0];
    assert_eq!(
        shared.dirs,
        ids(&["claude", "gemini"]),
        "one place, two names"
    );
    assert_eq!(shared.path, real(&claude_skills));
    assert_eq!(shared.tools, ids(&["claude-code", "gemini-cli"]));
    assert_eq!(
        shared.git_root.as_deref(),
        Some(real(&claude_skills).as_path())
    );
    assert!(!shared.create);

    let neutral = &plan.targets[1];
    assert_eq!(neutral.dirs, ids(&["agents"]));
    assert_eq!(neutral.tools, ids(&["codex", "gemini-cli"]));
    assert!(neutral.create);
    assert_eq!(neutral.git_root, None);

    assert_eq!(plan.duplicated, ids(&["gemini-cli"]));
}

/// One case of the selection rule: what is installed, what is chosen, and
/// which tools see the skill twice anyway.
struct Case {
    installed: &'static [&'static str],
    targets: &'static [&'static [&'static str]],
    duplicated: &'static [&'static str],
}

/// M2-A3: the selection rule of D3, case by case.
#[test]
fn the_fewest_copies_that_reach_every_tool_are_chosen() {
    let cases = [
        // Claude Code alone reads only its own directory.
        Case {
            installed: &["claude-code"],
            targets: &[&["claude"]],
            duplicated: &[],
        },
        // Gemini does not read ~/.claude/skills, so a second copy is needed;
        // the neutral directory wins the tie with ~/.gemini/skills.
        Case {
            installed: &["claude-code", "gemini-cli"],
            targets: &[&["claude"], &["agents"]],
            duplicated: &[],
        },
        // Cursor reads ~/.claude/skills, so one copy serves both.
        Case {
            installed: &["claude-code", "cursor"],
            targets: &[&["claude"]],
            duplicated: &[],
        },
        // Codex reads only ~/.agents/skills: two copies, and Cursor reads both.
        Case {
            installed: &["claude-code", "codex", "cursor"],
            targets: &[&["claude"], &["agents"]],
            duplicated: &["cursor"],
        },
        // Two tools that share the neutral directory need nothing else.
        Case {
            installed: &["gemini-cli", "copilot"],
            targets: &[&["agents"]],
            duplicated: &[],
        },
        // Copilot in VS Code may read ~/.claude/skills as well, so next to
        // Claude Code a duplicate cannot be avoided; it is named.
        Case {
            installed: &["claude-code", "copilot"],
            targets: &[&["claude"], &["agents"]],
            duplicated: &["copilot"],
        },
        // Every tool at once. Gemini is not among the duplicates: it reads
        // ~/.gemini/skills and ~/.agents/skills, and only the second is used.
        Case {
            installed: &[
                "claude-code",
                "codex",
                "gemini-cli",
                "copilot",
                "cursor",
                "opencode",
            ],
            targets: &[&["claude"], &["agents"]],
            duplicated: &["copilot", "cursor", "opencode"],
        },
    ];
    for case in cases {
        let home = machine(case.installed);
        let expected: Vec<Vec<String>> = case.targets.iter().map(|dirs| ids(dirs)).collect();
        assert_eq!(
            chosen(home.path()),
            (expected, ids(case.duplicated)),
            "installed: {:?}",
            case.installed
        );
    }
}

/// M2-A3, counter-probe: an empty machine gets an empty plan rather than a
/// directory nobody reads.
#[test]
fn nothing_installed_means_nothing_planned() {
    let home = machine(&[]);
    assert_eq!(chosen(home.path()), (Vec::new(), Vec::new()));
}

/// A relocating variable moves the target with it.
#[test]
fn claude_config_dir_moves_the_target() {
    let home = tempfile::tempdir().expect("home");
    let relocated = home.path().join("elsewhere");
    fs::create_dir_all(&relocated).expect("relocated home");
    let env = Environment::new(home.path()).with_var("CLAUDE_CONFIG_DIR", &relocated);
    let plan = plan(&Catalog::builtin(), &env).expect("plan");
    assert_eq!(plan.installed, ids(&["claude-code"]));
    assert_eq!(plan.targets[0].path, real(&relocated).join("skills"));
}

/// A tool whose directory cannot be resolved is reported, not silently
/// skipped and not written through.
#[cfg(unix)]
#[test]
fn a_dangling_symlink_leaves_its_tool_unserved_with_a_reason() {
    let home = machine(&["claude-code"]);
    std::os::unix::fs::symlink(home.path().join("gone"), home.path().join(".claude/skills"))
        .expect("dangling link");
    let plan = plan(&Catalog::builtin(), &Environment::new(home.path())).expect("plan");
    assert!(plan.targets.is_empty(), "{:?}", plan.targets);
    assert_eq!(plan.unserved.len(), 1);
    assert_eq!(plan.unserved[0].tool, "claude-code");
    assert!(
        plan.unserved[0].reason.contains("dangling"),
        "{}",
        plan.unserved[0].reason
    );
}

/// M2-A4: a target inside a git working tree names the root, also when the
/// repository is further up and when `.git` is a file, as in a linked
/// worktree.
#[test]
fn a_git_working_tree_is_reported_with_its_root() {
    let home = machine(&["claude-code"]);
    fs::write(home.path().join(".git"), "gitdir: /elsewhere\n").expect(".git file");
    let plan = plan(&Catalog::builtin(), &Environment::new(home.path())).expect("plan");
    assert_eq!(plan.targets[0].git_root, Some(real(home.path())));
}

/// M2-A4, counter-probe: the same layout without `.git` reports nothing.
#[test]
fn no_git_working_tree_reports_nothing() {
    let home = machine(&["claude-code"]);
    let plan = plan(&Catalog::builtin(), &Environment::new(home.path())).expect("plan");
    // The temporary directory itself must not lie in a repository either,
    // or this would test the machine running it.
    assert_eq!(
        plan.targets[0].git_root, None,
        "temp dir inside a repository?"
    );
}

/// M2-A5: a tool a program adds is planned like a built-in one.
#[test]
fn an_added_tool_is_treated_like_a_builtin_one() {
    let home = tempfile::tempdir().expect("home");
    fs::create_dir_all(home.path().join(".kiro")).expect("kiro home");
    let mut catalog = Catalog::builtin();
    catalog
        .add_dir(
            SkillDir::new("kiro", Location::home(".kiro/skills")).requires(Location::home(".kiro")),
        )
        .expect("dir")
        .add_tool(Tool::new("kiro", "Kiro", Location::home(".kiro")).reads(["kiro"]))
        .expect("tool");

    let plan = plan(&catalog, &Environment::new(home.path())).expect("plan");
    assert_eq!(plan.installed, ids(&["kiro"]));
    assert_eq!(plan.targets.len(), 1);
    assert_eq!(plan.targets[0].dirs, ids(&["kiro"]));
    assert_eq!(plan.targets[0].tools, ids(&["kiro"]));
}

/// M2-A5: what a program adds is checked the way the built-in list is.
#[test]
fn a_catalog_refuses_entries_that_would_mislead_the_planner() {
    let mut catalog = Catalog::builtin();
    let claude_again = SkillDir::new("claude", Location::home("x"));
    assert_eq!(
        catalog.add_dir(claude_again).err(),
        Some(CatalogError::DuplicateId("claude".into()))
    );

    let reads = |reads: Vec<String>| Tool::new("t", "T", Location::home(".t")).reads(reads);
    assert_eq!(
        catalog.add_tool(reads(vec![])).err(),
        Some(CatalogError::ReadsNothing("t".into()))
    );
    assert_eq!(
        catalog.add_tool(reads(vec!["nowhere".into()])).err(),
        Some(CatalogError::UnknownDir {
            tool: "t".into(),
            dir: "nowhere".into()
        })
    );
}

/// The built-in list passes the same checks a program's additions do.
#[test]
fn the_builtin_catalog_is_consistent() {
    let builtin = Catalog::builtin();
    let mut rebuilt = Catalog::empty();
    for dir in builtin.dirs() {
        rebuilt.add_dir(dir.clone()).expect("builtin dir");
    }
    for tool in builtin.tools() {
        rebuilt.add_tool(tool.clone()).expect("builtin tool");
    }
    assert_eq!(rebuilt, builtin);
}

/// A catalog of plain directories below `.t/` and tools reading them, for
/// cases the built-in list cannot isolate.
fn custom(dirs: &[(&str, bool)], tools: &[(&str, &[&str])]) -> Catalog {
    let mut catalog = Catalog::empty();
    for (id, preferred) in dirs {
        catalog
            .add_dir({
                let dir = SkillDir::new(*id, Location::home(format!(".t/{id}")));
                if *preferred { dir.preferred() } else { dir }
            })
            .expect("dir");
    }
    for (id, reads) in tools {
        catalog
            .add_tool(Tool::new(*id, *id, Location::home(".t")).reads(ids(reads)))
            .expect("tool");
    }
    catalog
}

/// M2-A3: avoiding a duplicate outranks having fewer copies.
///
/// `x1` and `x2` are one place through a symlink. One copy there reaches both
/// tools, but `t` reads both names and would see it twice; two copies, in `y`
/// and `z`, show each tool the skill once. Fewer duplicates wins over fewer
/// copies.
#[cfg(unix)]
#[test]
fn a_duplicate_is_avoided_before_copies_are_saved() {
    let home = tempfile::tempdir().expect("home");
    let h = home.path();
    for dir in ["x1", "y", "z"] {
        fs::create_dir_all(h.join(".t").join(dir)).expect("dir");
    }
    std::os::unix::fs::symlink(h.join(".t/x1"), h.join(".t/x2")).expect("link");
    let catalog = custom(
        &[("x1", false), ("x2", false), ("y", false), ("z", false)],
        &[("t", &["x1", "x2", "y"]), ("u", &["x1", "z"])],
    );

    let plan = plan(&catalog, &Environment::new(h)).expect("plan");
    let chosen: Vec<Vec<String>> = plan.targets.iter().map(|t| t.dirs.clone()).collect();
    assert_eq!(chosen, vec![ids(&["y"]), ids(&["z"])]);
    assert!(plan.duplicated.is_empty(), "{:?}", plan.duplicated);
}

/// M2-A3: between two otherwise equal choices the preferred directory wins,
/// even when it comes later in the catalog.
#[test]
fn the_preferred_directory_wins_a_tie() {
    let home = tempfile::tempdir().expect("home");
    fs::create_dir_all(home.path().join(".t")).expect("tool home");
    let catalog = custom(
        &[("first", false), ("second", true)],
        &[("t", &["first", "second"])],
    );
    let plan = plan(&catalog, &Environment::new(home.path())).expect("plan");
    assert_eq!(plan.targets.len(), 1);
    assert_eq!(plan.targets[0].dirs, ids(&["second"]));
}

/// M2-A1: a directory whose tool home is missing is not a candidate, even
/// when it is the only directory an installed tool reads. The tool is
/// reported as unserved instead of getting someone else's home created.
#[test]
fn a_missing_home_is_never_created_to_serve_a_tool() {
    let home = tempfile::tempdir().expect("home");
    fs::create_dir_all(home.path().join(".t")).expect("tool home");
    let mut catalog = Catalog::empty();
    catalog
        .add_dir(
            SkillDir::new("theirs", Location::home(".other/skills"))
                .requires(Location::home(".other")),
        )
        .expect("dir")
        .add_tool(Tool::new("t", "T", Location::home(".t")).reads(ids(&["theirs"])))
        .expect("tool");

    let plan = plan(&catalog, &Environment::new(home.path())).expect("plan");
    assert!(plan.targets.is_empty(), "{:?}", plan.targets);
    assert_eq!(plan.unserved.len(), 1);
    assert!(
        plan.unserved[0].reason.contains("does not exist"),
        "{}",
        plan.unserved[0].reason
    );
}
