// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! The everyday layer, `AgentSkill`, against whole machines in temporary
//! directories.

use std::fs;
use std::path::Path;

use agent_skill_installer::{AgentSkill, Done, Environment, Program, Skill};

fn agent(dir: &Path, version: &str) -> AgentSkill {
    let skill = Skill::new("devserial", "Serial terminal.", "body\n").expect("skill");
    let program = Program::new("devserial", version).expect("program");
    AgentSkill::new(skill, program, dir.join("consent"))
        .expect("home")
        .with_environment(Environment::new(dir.join("home")))
}

fn tool(dir: &Path, home: &str) {
    fs::create_dir_all(dir.join("home").join(home)).expect("tool");
}

/// An undecided place is offered; accepting installs, and then nothing is
/// offered any more.
#[test]
fn an_accepted_offer_installs_and_is_not_made_again() {
    let dir = tempfile::tempdir().expect("dir");
    tool(dir.path(), ".claude");
    let agent = agent(dir.path(), "1.0.0");

    let offer = agent.offer().expect("offer").expect("a place to offer");
    assert_eq!(offer.steps().len(), 1);
    assert_eq!(offer.steps()[0].tools, ["claude-code"]);
    let outcomes = offer.accept().expect("accept");
    assert_eq!(outcomes[0].done, Done::Installed);
    assert!(
        dir.path()
            .join("home/.claude/skills/devserial/SKILL.md")
            .exists()
    );
    assert!(agent.offer().expect("offer").is_none());
}

/// A declined place is not offered again; a tool found later is offered on
/// its own; an explicit install still goes everywhere.
#[test]
fn a_declined_offer_is_remembered() {
    let dir = tempfile::tempdir().expect("dir");
    tool(dir.path(), ".claude");
    let agent = agent(dir.path(), "1.0.0");

    agent
        .offer()
        .expect("offer")
        .expect("offered")
        .decline()
        .expect("decline");
    assert!(agent.offer().expect("offer").is_none(), "offered again");

    tool(dir.path(), ".codex");
    let later = agent.offer().expect("offer").expect("the new place");
    assert_eq!(later.steps().len(), 1);
    assert!(later.steps()[0].place.ends_with(".agents/skills"));
    drop(later);

    let outcomes = agent.install_everywhere().expect("install");
    assert_eq!(outcomes.len(), 2, "{outcomes:?}");
    assert!(
        outcomes
            .iter()
            .all(|outcome| outcome.done == Done::Installed)
    );
    assert!(agent.offer().expect("offer").is_none());
}

/// An unchanged older copy is updated without asking; removing takes out
/// what the program installed.
#[test]
fn an_older_copy_is_updated_and_can_be_removed() {
    let dir = tempfile::tempdir().expect("dir");
    tool(dir.path(), ".claude");
    let old = agent(dir.path(), "1.0.0");
    old.offer()
        .expect("offer")
        .expect("offered")
        .accept()
        .expect("accept");

    let new = agent(dir.path(), "1.1.0");
    let updated = new.update().expect("update");
    assert_eq!(
        updated[0].done,
        Done::Updated {
            from: semver::Version::new(1, 0, 0)
        }
    );
    assert!(new.update().expect("again").is_empty(), "updated twice");

    let removed = new.remove().expect("remove");
    assert_eq!(removed[0].done, Done::Removed);
    assert!(!dir.path().join("home/.claude/skills/devserial").exists());
}

/// Nothing to offer on a machine without AI tools.
#[test]
fn without_tools_there_is_nothing_to_offer() {
    let dir = tempfile::tempdir().expect("dir");
    fs::create_dir_all(dir.path().join("home")).expect("home");
    assert!(agent(dir.path(), "1.0.0").offer().expect("offer").is_none());
}
