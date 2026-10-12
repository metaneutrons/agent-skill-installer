// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! M3 of docs/plans/agent-skill-installer.md: installing, updating and
//! removing, against whole machines in temporary directories.

use std::fs;
use std::path::{Path, PathBuf};

use agent_skill_installer::{
    Action, Approval, Catalog, Consent, Done, Environment, Found, Installer, Problem, Program,
    SKILL_FILE, Skill, Why,
};

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

fn machine(tools: &[&str]) -> tempfile::TempDir {
    let home = tempfile::tempdir().expect("temp home");
    for dir in tools {
        fs::create_dir_all(home.path().join(dir)).expect("tool home");
    }
    home
}

fn skill(body: &str) -> Skill {
    Skill::new(
        "devserial",
        "Serial terminal with a SQLite-buffered capture.",
        body,
    )
    .expect("skill")
    .with_field("license", "GPL-3.0-or-later")
    .expect("field")
    .with_file("references/cli.md", "# CLI\n")
    .expect("file")
}

fn program(version: &str) -> Program {
    Program::new("devserial", version).expect("program")
}

fn no_consent(home: &Path) -> Consent {
    Consent::load(home.join("consent")).expect("consent")
}

/// Install with every target approved, as a user saying yes would.
fn install(home: &Path, skill: &Skill, program: &Program) -> Vec<PathBuf> {
    let catalog = Catalog::builtin();
    let env = Environment::new(home);
    let installer = Installer::new(&catalog, &env, skill, program);
    let assessment = installer.assess(&no_consent(home)).expect("assess");
    let approval = Approval::install(
        assessment
            .awaiting_approval()
            .map(|step| step.place.clone()),
    );
    let outcomes = installer.apply(&assessment, &approval);
    assert!(
        outcomes
            .iter()
            .all(|outcome| outcome.done == Done::Installed),
        "{outcomes:?}"
    );
    outcomes.into_iter().map(|outcome| outcome.place).collect()
}

/// The first step of a fresh assessment.
fn first_step(home: &Path, skill: &Skill, program: &Program) -> agent_skill_installer::Step {
    let catalog = Catalog::builtin();
    let env = Environment::new(home);
    let installer = Installer::new(&catalog, &env, skill, program);
    installer
        .assess(&no_consent(home))
        .expect("assess")
        .steps
        .into_iter()
        .next()
        .expect("a step")
}

fn installed_version(skill_dir: &Path) -> String {
    let text = fs::read_to_string(skill_dir.join(SKILL_FILE)).expect("SKILL.md");
    text.lines()
        .find_map(|line| line.strip_prefix("  installed-version: "))
        .expect("version line")
        .trim_matches('"')
        .to_string()
}

// ------------------------------------------------------------------ M3-A1

/// The record is written, read back, and an untouched skill is unmodified.
#[test]
fn an_installed_skill_is_recognised_as_ours_and_unmodified() {
    let home = machine(&[".claude"]);
    let places = install(home.path(), &skill("Use the CLI.\n"), &program("0.1.0"));
    let skill_md = fs::read_to_string(places[0].join("devserial").join(SKILL_FILE)).expect("read");
    assert!(
        skill_md.starts_with("---\nname: \"devserial\"\n"),
        "{skill_md}"
    );
    assert!(
        skill_md.contains("  installed-by: \"devserial\"\n"),
        "{skill_md}"
    );
    assert!(
        skill_md.contains("  content-digest: \"sha256:"),
        "{skill_md}"
    );

    let step = first_step(home.path(), &skill("Use the CLI.\n"), &program("0.1.0"));
    assert_eq!(
        step.found,
        Found::Ours {
            version: semver::Version::new(0, 1, 0),
            unmodified: true
        }
    );
}

/// Any change to any file makes the skill modified; a file manager's litter
/// does not.
#[test]
fn every_kind_of_change_is_seen_and_litter_is_not() {
    type Change = fn(&Path);
    let changes: [(&str, Change, bool); 9] = [
        ("untouched", |_| {}, true),
        // Same length: the digest has to cover content, not only sizes.
        (
            "a letter replaced in SKILL.md",
            |dir| replace(&dir.join(SKILL_FILE), "CLI.", "CLX."),
            false,
        ),
        (
            "a letter replaced in another file",
            |dir| replace(&dir.join("references/cli.md"), "CLI", "CLX"),
            false,
        ),
        (
            "a byte in SKILL.md's body",
            |dir| append(&dir.join(SKILL_FILE), "x"),
            false,
        ),
        (
            "a byte in another file",
            |dir| append(&dir.join("references/cli.md"), "x"),
            false,
        ),
        (
            "an added file",
            |dir| fs::write(dir.join("notes.md"), "mine").expect("write"),
            false,
        ),
        (
            "a removed file",
            |dir| fs::remove_file(dir.join("references/cli.md")).expect("rm"),
            false,
        ),
        (
            "the digest itself",
            |dir| replace(&dir.join(SKILL_FILE), "sha256:", "sha256:0"),
            false,
        ),
        (
            ".DS_Store from a glance in Finder",
            |dir| fs::write(dir.join(".DS_Store"), "x").expect("write"),
            true,
        ),
    ];
    for (what, change, unmodified) in changes {
        let home = machine(&[".claude"]);
        let places = install(home.path(), &skill("Use the CLI.\n"), &program("0.1.0"));
        change(&places[0].join("devserial"));
        let step = first_step(home.path(), &skill("Use the CLI.\n"), &program("0.1.0"));
        assert_eq!(
            step.found,
            Found::Ours {
                version: semver::Version::new(0, 1, 0),
                unmodified
            },
            "{what}"
        );
    }
}

fn append(path: &Path, text: &str) {
    let mut content = fs::read_to_string(path).expect("read");
    content.push_str(text);
    fs::write(path, content).expect("write");
}

fn replace(path: &Path, from: &str, to: &str) {
    let content = fs::read_to_string(path)
        .expect("read")
        .replacen(from, to, 1);
    fs::write(path, content).expect("write");
}

/// A value cannot break out of its quotes and forge the record.
#[test]
fn a_hostile_description_cannot_forge_the_record() {
    let hostile = Skill::new(
        "devserial",
        "x\"\n  installed-by: \"someone-else\"\n---\nname: evil",
        "body",
    )
    .expect("skill");
    let home = machine(&[".claude"]);
    let places = install(home.path(), &hostile, &program("0.1.0"));
    let step = first_step(home.path(), &hostile, &program("0.1.0"));
    assert!(
        matches!(
            step.found,
            Found::Ours {
                unmodified: true,
                ..
            }
        ),
        "{:?}",
        step.found
    );
    let text = fs::read_to_string(places[0].join("devserial").join(SKILL_FILE)).expect("read");
    // The description is one line in quotes; only the record starts a line
    // with the key.
    let record_lines = text
        .lines()
        .filter(|line| line.starts_with("  installed-by:"))
        .count();
    assert_eq!(record_lines, 1, "{text}");
    assert_eq!(
        text.lines().filter(|line| *line == "---").count(),
        2,
        "{text}"
    );
}

// ------------------------------------------------------------------ M3-A2

/// D5 row 1: an eligible target with nothing in it is installed only after
/// approval.
#[test]
fn nothing_is_installed_without_approval() {
    let home = machine(&[".claude"]);
    let catalog = Catalog::builtin();
    let env = Environment::new(home.path());
    let (skill, program) = (skill("x"), program("0.1.0"));
    let installer = Installer::new(&catalog, &env, &skill, &program);

    let assessment = installer.assess(&no_consent(home.path())).expect("assess");
    assert_eq!(assessment.steps[0].action, Action::Install);
    let outcomes = installer.apply(&assessment, &Approval::default());
    assert!(outcomes.is_empty(), "{outcomes:?}");
    assert!(
        !assessment.steps[0].skill_dir.exists(),
        "written without approval"
    );
}

/// D5 row 2: a declined target is left alone and not asked about again, and a
/// newly detected target is asked about on its own.
#[test]
fn a_declined_target_stays_declined_and_a_new_one_is_still_asked() {
    let home = machine(&[".claude"]);
    let (skill, program) = (skill("x"), program("0.1.0"));
    let catalog = Catalog::builtin();
    let env = Environment::new(home.path());
    let installer = Installer::new(&catalog, &env, &skill, &program);

    let mut consent = no_consent(home.path());
    let first = installer.assess(&consent).expect("assess");
    consent.decline(&first.steps[0].place).expect("decline");
    consent.save().expect("save");

    // A later run, a new process: the decline is read back from the file.
    fs::create_dir_all(home.path().join(".codex")).expect("codex appears");
    let consent = no_consent(home.path());
    let later = installer.assess(&consent).expect("assess");
    let claude = later
        .steps
        .iter()
        .find(|s| s.place == first.steps[0].place)
        .expect("claude");
    let agents = later
        .steps
        .iter()
        .find(|s| s.place.ends_with(".agents/skills"))
        .expect("agents");
    assert_eq!(claude.action, Action::Nothing(Why::Declined));
    assert_eq!(agents.action, Action::Install);
}

/// D5 row 3: an unchanged older copy is updated without asking, and the
/// record says so afterwards.
#[test]
fn an_unchanged_older_copy_is_updated_without_asking() {
    let home = machine(&[".claude"]);
    let places = install(home.path(), &skill("old"), &program("0.1.0"));
    let catalog = Catalog::builtin();
    let env = Environment::new(home.path());
    let (newer, program) = (skill("new"), program("0.2.0"));
    let outcomes = Installer::new(&catalog, &env, &newer, &program)
        .update()
        .expect("update");

    assert_eq!(outcomes.len(), 1, "{outcomes:?}");
    assert_eq!(
        outcomes[0].done,
        Done::Updated {
            from: semver::Version::new(0, 1, 0)
        }
    );
    let dir = places[0].join("devserial");
    assert_eq!(installed_version(&dir), "0.2.0");
    assert!(
        fs::read_to_string(dir.join(SKILL_FILE))
            .expect("read")
            .ends_with("new\n")
    );
    // And it is unmodified under the new version.
    let step = first_step(home.path(), &newer, &program);
    assert_eq!(step.action, Action::Nothing(Why::Current));
}

/// D5 row 4: the same version is left alone, and an older program never
/// downgrades a newer copy.
#[test]
fn a_current_or_newer_copy_is_left_alone() {
    let home = machine(&[".claude"]);
    let places = install(home.path(), &skill("v2"), &program("0.2.0"));
    let catalog = Catalog::builtin();
    let env = Environment::new(home.path());

    let (same, current) = (skill("v2"), program("0.2.0"));
    let outcomes = Installer::new(&catalog, &env, &same, &current)
        .update()
        .expect("update");
    assert!(outcomes.is_empty(), "{outcomes:?}");

    let (older, old) = (skill("v1"), program("0.1.0"));
    let step = first_step(home.path(), &older, &old);
    assert_eq!(
        step.action,
        Action::Nothing(Why::Newer(semver::Version::new(0, 2, 0)))
    );
    let outcomes = Installer::new(&catalog, &env, &older, &old)
        .update()
        .expect("update");
    assert!(outcomes.is_empty(), "{outcomes:?}");
    assert_eq!(installed_version(&places[0].join("devserial")), "0.2.0");
}

/// D5 row 5: a modified copy is reported, survives an update, and is replaced
/// only on an explicit overwrite.
#[test]
fn a_modified_copy_is_kept_until_overwritten_on_purpose() {
    let home = machine(&[".claude"]);
    let places = install(home.path(), &skill("old"), &program("0.1.0"));
    let dir = places[0].join("devserial");
    append(&dir.join(SKILL_FILE), "my own note\n");

    let catalog = Catalog::builtin();
    let env = Environment::new(home.path());
    let (newer, program) = (skill("new"), program("0.2.0"));
    let installer = Installer::new(&catalog, &env, &newer, &program);

    let outcomes = installer.update().expect("update");
    assert!(outcomes.is_empty(), "{outcomes:?}");
    assert!(
        fs::read_to_string(dir.join(SKILL_FILE))
            .expect("read")
            .contains("my own note")
    );

    let assessment = installer.assess(&no_consent(home.path())).expect("assess");
    assert_eq!(
        assessment.steps[0].action,
        Action::Report(Problem::Modified)
    );
    let outcomes = installer.apply(
        &assessment,
        &Approval::install(Vec::new()).with_overwrite(vec![places[0].clone()]),
    );
    assert_eq!(outcomes[0].done, Done::Overwritten);
    assert!(
        !fs::read_to_string(dir.join(SKILL_FILE))
            .expect("read")
            .contains("my own note")
    );
}

/// D5 row 6: a skill of the same name that is not ours is never replaced,
/// not by an update and not by an approval of any kind.
#[test]
fn a_skill_that_is_not_ours_is_never_replaced() {
    for (what, content) in [
        (
            "no record",
            "---\nname: devserial\ndescription: mine\n---\nmine\n",
        ),
        (
            "another program's record",
            "---\nname: devserial\ndescription: x\nmetadata:\n  installed-by: \"devknx\"\n  installed-version: \"1.0.0\"\n  content-digest: \"sha256:00\"\n---\nmine\n",
        ),
    ] {
        let home = machine(&[".claude"]);
        let dir = home.path().join(".claude/skills/devserial");
        fs::create_dir_all(&dir).expect("dir");
        fs::write(dir.join(SKILL_FILE), content).expect("write");

        let catalog = Catalog::builtin();
        let env = Environment::new(home.path());
        let (skill, program) = (skill("ours"), program("0.1.0"));
        let installer = Installer::new(&catalog, &env, &skill, &program);
        let assessment = installer.assess(&no_consent(home.path())).expect("assess");
        assert!(
            matches!(
                assessment.steps[0].action,
                Action::Report(Problem::Foreign(_))
            ),
            "{what}: {:?}",
            assessment.steps[0].action
        );
        let everything = Approval::install(vec![assessment.steps[0].place.clone()])
            .with_overwrite(vec![assessment.steps[0].place.clone()]);
        assert!(
            installer.apply(&assessment, &everything).is_empty(),
            "{what}"
        );
        assert!(installer.update().expect("update").is_empty(), "{what}");
        assert_eq!(
            fs::read_to_string(dir.join(SKILL_FILE)).expect("read"),
            content,
            "{what}"
        );
    }
}

/// D6: an update proceeds inside a git working tree, and the step names it.
#[test]
fn an_update_proceeds_inside_a_git_working_tree() {
    let home = machine(&[".claude"]);
    fs::create_dir_all(home.path().join(".claude/skills/.git")).expect("repository");
    let places = install(home.path(), &skill("old"), &program("0.1.0"));
    let step = first_step(home.path(), &skill("new"), &program("0.2.0"));
    assert_eq!(
        step.git_root,
        Some(real(&home.path().join(".claude/skills")))
    );

    let catalog = Catalog::builtin();
    let env = Environment::new(home.path());
    let (newer, program) = (skill("new"), program("0.2.0"));
    let outcomes = Installer::new(&catalog, &env, &newer, &program)
        .update()
        .expect("update");
    assert_eq!(outcomes.len(), 1, "{outcomes:?}");
    assert_eq!(installed_version(&places[0].join("devserial")), "0.2.0");
}

/// A copy in a place no installed tool reads any more is still ours: kept
/// current and removable.
#[test]
fn a_copy_outside_the_plan_is_still_updated_and_removed() {
    let home = machine(&[".claude", ".codex"]);
    let places = install(home.path(), &skill("old"), &program("0.1.0"));
    assert_eq!(places.len(), 2);
    fs::remove_dir_all(home.path().join(".codex")).expect("codex uninstalled");

    let catalog = Catalog::builtin();
    let env = Environment::new(home.path());
    let (newer, program) = (skill("new"), program("0.2.0"));
    let installer = Installer::new(&catalog, &env, &newer, &program);
    let updated = installer.update().expect("update");
    assert_eq!(updated.len(), 2, "{updated:?}");

    let removed = installer.remove().expect("remove");
    assert_eq!(removed.len(), 2, "{removed:?}");
    assert!(removed.iter().all(|outcome| outcome.done == Done::Removed));
    for place in &places {
        assert!(!place.join("devserial").exists(), "{}", place.display());
    }
}

/// Removal takes only unchanged copies of our own.
#[test]
fn removal_leaves_a_modified_copy_alone() {
    let home = machine(&[".claude"]);
    let places = install(home.path(), &skill("x"), &program("0.1.0"));
    append(&places[0].join("devserial").join(SKILL_FILE), "mine\n");
    let catalog = Catalog::builtin();
    let env = Environment::new(home.path());
    let (skill, program) = (skill("x"), program("0.1.0"));
    let removed = Installer::new(&catalog, &env, &skill, &program)
        .remove()
        .expect("remove");
    assert!(removed.is_empty(), "{removed:?}");
    assert!(places[0].join("devserial").exists());
}

// ------------------------------------------------------------------ M3-A4

/// A skill directory that is a symlink is never written through, and what it
/// points to is not touched.
#[cfg(unix)]
#[test]
fn a_symlinked_skill_directory_is_never_written_through() {
    let home = machine(&[".claude"]);
    let elsewhere = home.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).expect("elsewhere");
    fs::write(elsewhere.join("keep.txt"), "keep").expect("write");
    fs::create_dir_all(home.path().join(".claude/skills")).expect("skills");
    std::os::unix::fs::symlink(&elsewhere, home.path().join(".claude/skills/devserial"))
        .expect("link");

    let catalog = Catalog::builtin();
    let env = Environment::new(home.path());
    let (skill, program) = (skill("x"), program("0.1.0"));
    let installer = Installer::new(&catalog, &env, &skill, &program);
    let assessment = installer.assess(&no_consent(home.path())).expect("assess");
    assert_eq!(
        assessment.steps[0].found,
        Found::Foreign("is a symlink".into())
    );
    let everything = Approval::install(vec![assessment.steps[0].place.clone()])
        .with_overwrite(vec![assessment.steps[0].place.clone()]);
    let outcomes = installer.apply(&assessment, &everything);
    assert!(outcomes.is_empty(), "{outcomes:?}");
    assert_eq!(fs::read_dir(&elsewhere).expect("read").count(), 1);
}

/// A symlink planted inside an installed copy makes it modified, so no update
/// follows it.
#[cfg(unix)]
#[test]
fn a_symlink_inside_our_copy_stops_updates() {
    let home = machine(&[".claude"]);
    let places = install(home.path(), &skill("x"), &program("0.1.0"));
    std::os::unix::fs::symlink("/etc", places[0].join("devserial/etc")).expect("link");
    let step = first_step(home.path(), &skill("y"), &program("0.2.0"));
    assert_eq!(step.action, Action::Report(Problem::Modified));
}

/// Names and file paths that could leave the skill directory are refused
/// before anything is planned.
#[test]
fn paths_that_could_leave_the_skill_are_refused() {
    assert!(Skill::new("..", "d", "b").is_err());
    assert!(Skill::new("a/b", "d", "b").is_err());
    let base = || Skill::new("devserial", "d", "b").expect("skill");
    for path in [
        "/etc/passwd",
        "../outside",
        "a/../../b",
        "./x",
        "",
        "a\\b",
        "SKILL.md",
    ] {
        assert!(
            base().with_file(path, "x").is_err(),
            "{path:?} must be refused"
        );
    }
    assert!(base().with_file("references/ok.md", "x").is_ok());
}

/// Keys that would collide with what the crate writes are refused.
#[test]
fn reserved_keys_are_refused() {
    let base = || Skill::new("devserial", "d", "b").expect("skill");
    for key in ["name", "description", "metadata", "Upper", "with space", ""] {
        assert!(base().with_field(key, "v").is_err(), "{key:?}");
    }
    for key in ["installed-by", "installed-version", "content-digest"] {
        assert!(base().with_metadata(key, "v").is_err(), "{key:?}");
    }
    assert!(base().with_metadata("opencode/slash", "true").is_ok());
}

// ------------------------------------------------------------------ M3-A5

/// The consent store lives where the caller says, survives a reload, and can
/// be withdrawn.
#[test]
fn declines_are_kept_in_the_callers_file() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("data/devserial/skill-consent");
    let target = Path::new("/home/someone/.claude/skills");

    let mut consent = Consent::load(&file).expect("load");
    assert!(!consent.is_declined(target));
    consent.decline(target).expect("decline");
    consent.save().expect("save");
    assert!(file.exists());

    let mut reloaded = Consent::load(&file).expect("reload");
    assert!(reloaded.is_declined(target));
    reloaded.withdraw(target);
    reloaded.save().expect("save");
    assert!(!Consent::load(&file).expect("reload").is_declined(target));

    assert!(consent.decline(Path::new("/a\nb")).is_err());
}

// ------------------------------------------------------- since the assessment

/// Siblings of the skills directory the installer stages in.
fn staging_left(place: &Path) -> Vec<String> {
    let parent = place.parent().expect("parent");
    fs::read_dir(parent)
        .expect("read")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.starts_with(".asi-"))
        .collect()
}

/// A copy a person edits between the assessment and the update is not
/// overwritten, whatever the assessment said.
#[test]
fn a_skill_edited_after_the_assessment_is_not_overwritten() {
    let home = machine(&[".claude"]);
    let places = install(home.path(), &skill("old"), &program("0.1.0"));
    let catalog = Catalog::builtin();
    let env = Environment::new(home.path());
    let (newer, program) = (skill("new"), program("0.2.0"));
    let installer = Installer::new(&catalog, &env, &newer, &program);
    let assessment = installer.assess(&no_consent(home.path())).expect("assess");
    assert!(
        matches!(assessment.steps[0].action, Action::Update { .. }),
        "{:?}",
        assessment.steps[0].action
    );

    let skill_md = places[0].join("devserial").join(SKILL_FILE);
    let edited = format!(
        "{}my own note\n",
        fs::read_to_string(&skill_md).expect("read")
    );
    fs::write(&skill_md, &edited).expect("edit");

    let outcomes = installer.apply(&assessment, &Approval::default());
    assert_eq!(outcomes.len(), 1, "{outcomes:?}");
    assert_eq!(outcomes[0].done, Done::Changed);
    assert_eq!(fs::read_to_string(&skill_md).expect("read"), edited);
    assert_eq!(staging_left(&places[0]), Vec::<String>::new());
}

/// A skill of somebody else's put into an empty place between the assessment
/// and the approved install is left alone.
#[test]
fn a_skill_put_in_place_after_the_assessment_is_left_alone() {
    let home = machine(&[".claude"]);
    let catalog = Catalog::builtin();
    let env = Environment::new(home.path());
    let (skill, program) = (skill("ours"), program("0.1.0"));
    let installer = Installer::new(&catalog, &env, &skill, &program);
    let assessment = installer.assess(&no_consent(home.path())).expect("assess");
    let place = assessment.steps[0].place.clone();
    assert_eq!(assessment.steps[0].action, Action::Install);

    let theirs = place.join("devserial");
    fs::create_dir_all(&theirs).expect("theirs");
    fs::write(theirs.join(SKILL_FILE), "theirs\n").expect("write");

    let approval = Approval::install(vec![place.clone()]);
    let outcomes = installer.apply(&assessment, &approval);
    assert_eq!(outcomes[0].done, Done::Changed, "{outcomes:?}");
    assert_eq!(
        fs::read_to_string(theirs.join(SKILL_FILE)).expect("read"),
        "theirs\n"
    );
    assert_eq!(staging_left(&place), Vec::<String>::new());
}
