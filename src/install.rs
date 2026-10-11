// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! Installing, updating and removing a skill (D4, D5, D7).
//!
//! [`Installer::assess`] looks at every place and says what it would do and
//! why. [`Installer::apply`] does what the user approved. [`Installer::update`]
//! is the part that needs no approval: a skill this program installed earlier,
//! unchanged since, brought up to the running version.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::catalog::Catalog;
use crate::consent::Consent;
use crate::env::Environment;
use crate::plan::{Plan, PlanError, existing_places, plan};
use crate::record;
use crate::resolve;
use crate::skill::{SKILL_FILE, Skill};

/// The program shipping the skill: its name goes into the record, and its
/// version decides whether an installed copy is older.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    name: String,
    version: semver::Version,
}

impl Program {
    /// A program and its version, such as `env!("CARGO_PKG_VERSION")`.
    ///
    /// # Errors
    /// Returns an error when the version is not semantic versioning.
    pub fn new(name: impl Into<String>, version: &str) -> Result<Self, semver::Error> {
        Ok(Self {
            name: name.into(),
            version: semver::Version::parse(version)?,
        })
    }
}

/// What is in a place under the skill's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// Nothing.
    Absent,
    /// A skill this program installed.
    Ours {
        /// The version that installed it.
        version: semver::Version,
        /// Whether every file is as it was installed.
        unmodified: bool,
    },
    /// Something that cannot be shown to be this program's, and why.
    Foreign(String),
}

/// What the installer would do in a place, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Install, once the user agrees.
    Install,
    /// Bring an unchanged installed copy up to the running version. Needs no
    /// approval.
    Update {
        /// The version installed now.
        from: semver::Version,
    },
    /// Nothing to do.
    Nothing(Why),
    /// Nothing is done; the user should know.
    Report(Problem),
}

/// Why nothing is done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    /// The user declined this target.
    Declined,
    /// The installed copy is this version.
    Current,
    /// The installed copy is newer than the running program, which never
    /// downgrades it.
    Newer(semver::Version),
    /// The place is not a planned target and holds nothing of this program.
    NotPlanned,
}

/// Why something needs the user's attention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The installed copy was changed after installation. Only an explicit
    /// overwrite replaces it.
    Modified,
    /// A skill of this name that is not this program's, or something that is
    /// not a skill directory at all. Never replaced.
    Foreign(String),
}

/// One place, what is there and what would happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The physical skills directory.
    pub place: PathBuf,
    /// The skill's directory inside it.
    pub skill_dir: PathBuf,
    /// Whether the plan chose this place.
    pub planned: bool,
    /// The installed tools reading a skill here, when planned.
    pub tools: Vec<String>,
    /// The root of the git working tree the place lies in, if any.
    pub git_root: Option<PathBuf>,
    /// What is there now.
    pub found: Found,
    /// What would happen.
    pub action: Action,
}

/// A plan and the step for every place it concerns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    /// Where the skill should go.
    pub plan: Plan,
    /// One step per planned target, then one per further place holding this
    /// program's skill.
    pub steps: Vec<Step>,
}

impl Assessment {
    /// Steps waiting for the user's approval.
    pub fn awaiting_approval(&self) -> impl Iterator<Item = &Step> {
        self.steps
            .iter()
            .filter(|step| step.action == Action::Install)
    }
}

/// What the user agreed to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Approval {
    /// Places to install into.
    pub install: Vec<PathBuf>,
    /// Places whose modified copy is to be replaced anyway.
    pub overwrite: Vec<PathBuf>,
}

/// What happened in one place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The physical skills directory.
    pub place: PathBuf,
    /// What happened there.
    pub done: Done,
}

/// What happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Done {
    /// Installed.
    Installed,
    /// Updated from an older version.
    Updated {
        /// The version that was there.
        from: semver::Version,
    },
    /// A modified copy was replaced on explicit request.
    Overwritten,
    /// Removed.
    Removed,
    /// Nothing was written: the place changed after the assessment, and what
    /// is there now was put back untouched. Assessing again shows what it is.
    Changed,
    /// Writing failed; the place is as it was before.
    Failed(String),
}

impl fmt::Display for Done {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Installed => f.write_str("installed"),
            Self::Updated { from } => write!(f, "updated from {from}"),
            Self::Overwritten => f.write_str("overwritten"),
            Self::Removed => f.write_str("removed"),
            Self::Changed => f.write_str("left alone: changed since the assessment"),
            Self::Failed(why) => write!(f, "failed: {why}"),
        }
    }
}

/// A skill, the program it belongs to, and the machine to put it on.
#[derive(Debug, Clone, Copy)]
pub struct Installer<'a> {
    catalog: &'a Catalog,
    env: &'a Environment,
    skill: &'a Skill,
    program: &'a Program,
}

impl<'a> Installer<'a> {
    /// An installer for one skill of one program.
    #[must_use]
    pub const fn new(
        catalog: &'a Catalog,
        env: &'a Environment,
        skill: &'a Skill,
        program: &'a Program,
    ) -> Self {
        Self {
            catalog,
            env,
            skill,
            program,
        }
    }

    /// What would happen, place by place. Writes nothing.
    ///
    /// # Errors
    /// Returns an error only when no plan can be made.
    pub fn assess(&self, consent: &Consent) -> Result<Assessment, PlanError> {
        self.assess_with(&|place| consent.is_declined(place))
    }

    fn assess_with(&self, declined: &dyn Fn(&Path) -> bool) -> Result<Assessment, PlanError> {
        let plan = plan(self.catalog, self.env)?;
        let mut steps: Vec<Step> = plan
            .targets
            .iter()
            .map(|target| {
                let skill_dir = target.path.join(self.skill.name());
                let found = self.found(&skill_dir);
                let action = self.action(&found, true, declined(&target.path));
                Step {
                    place: target.path.clone(),
                    skill_dir,
                    planned: true,
                    tools: target.tools.clone(),
                    git_root: target.git_root.clone(),
                    found,
                    action,
                }
            })
            .collect();

        for place in existing_places(self.catalog, self.env) {
            if steps.iter().any(|step| step.place == place) {
                continue;
            }
            let skill_dir = place.join(self.skill.name());
            let found = self.found(&skill_dir);
            if found == Found::Absent {
                continue;
            }
            let action = self.action(&found, false, false);
            steps.push(Step {
                git_root: resolve::git_root(&place),
                place,
                skill_dir,
                planned: false,
                tools: Vec::new(),
                found,
                action,
            });
        }
        Ok(Assessment { plan, steps })
    }

    /// Install where approved, overwrite where explicitly asked, and update
    /// whatever needs no approval.
    #[must_use]
    pub fn apply(&self, assessment: &Assessment, approval: &Approval) -> Vec<Outcome> {
        assessment
            .steps
            .iter()
            .filter_map(|step| {
                let done = match &step.action {
                    Action::Install if approval.install.contains(&step.place) => {
                        self.write(step, Done::Installed)
                    }
                    Action::Update { from } => {
                        self.write(step, Done::Updated { from: from.clone() })
                    }
                    Action::Report(Problem::Modified)
                        if approval.overwrite.contains(&step.place) =>
                    {
                        self.write(step, Done::Overwritten)
                    }
                    _ => return None,
                };
                Some(Outcome {
                    place: step.place.clone(),
                    done,
                })
            })
            .collect()
    }

    /// Bring every unchanged copy this program installed up to the running
    /// version. Installs nothing new and needs no approval.
    ///
    /// # Errors
    /// Returns an error only when no plan can be made.
    pub fn update(&self) -> Result<Vec<Outcome>, PlanError> {
        // Declines do not matter here: only an install asks for consent, and
        // an empty approval installs nothing.
        let assessment = self.assess_with(&|_| false)?;
        Ok(self.apply(&assessment, &Approval::default()))
    }

    /// Remove every unchanged copy this program installed. A modified copy
    /// and anything not this program's is left where it is.
    ///
    /// # Errors
    /// Returns an error only when no plan can be made.
    pub fn remove(&self) -> Result<Vec<Outcome>, PlanError> {
        let assessment = self.assess_with(&|_| false)?;
        Ok(assessment
            .steps
            .iter()
            .filter(|step| {
                matches!(
                    step.found,
                    Found::Ours {
                        unmodified: true,
                        ..
                    }
                )
            })
            .map(|step| Outcome {
                place: step.place.clone(),
                done: match remove_dir(&step.skill_dir) {
                    Ok(()) => Done::Removed,
                    Err(error) => Done::Failed(error.to_string()),
                },
            })
            .collect())
    }

    fn found(&self, skill_dir: &Path) -> Found {
        let meta = match fs::symlink_metadata(skill_dir) {
            Ok(meta) => meta,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Found::Absent,
            Err(error) => return Found::Foreign(error.to_string()),
        };
        if meta.file_type().is_symlink() {
            // Writing through it would leave the place; and a link somebody
            // set up is theirs, not ours (D7).
            return Found::Foreign("is a symlink".to_string());
        }
        if !meta.is_dir() {
            return Found::Foreign("is not a directory".to_string());
        }
        let Ok(skill_md) = fs::read(skill_dir.join(SKILL_FILE)) else {
            return Found::Foreign(format!("has no readable {SKILL_FILE}"));
        };
        let Some(record) = record::parse(&skill_md) else {
            return Found::Foreign("carries no installation record".to_string());
        };
        if record.installed_by != self.program.name {
            return Found::Foreign(format!("was installed by {}", record.installed_by));
        }
        let Ok(version) = semver::Version::parse(&record.installed_version) else {
            return Found::Foreign("has an unreadable version in its record".to_string());
        };
        let unmodified = record::read_dir(skill_dir)
            .is_ok_and(|files| record::digest(&files) == record.content_digest);
        Found::Ours {
            version,
            unmodified,
        }
    }

    fn action(&self, found: &Found, planned: bool, declined: bool) -> Action {
        match found {
            Found::Absent if !planned => Action::Nothing(Why::NotPlanned),
            Found::Absent if declined => Action::Nothing(Why::Declined),
            Found::Absent => Action::Install,
            Found::Ours {
                unmodified: false, ..
            } => Action::Report(Problem::Modified),
            Found::Ours { version, .. } if *version < self.program.version => Action::Update {
                from: version.clone(),
            },
            Found::Ours { version, .. } if *version > self.program.version => {
                Action::Nothing(Why::Newer(version.clone()))
            }
            Found::Ours { .. } => Action::Nothing(Why::Current),
            Found::Foreign(why) => Action::Report(Problem::Foreign(why.clone())),
        }
    }

    fn write(&self, step: &Step, done: Done) -> Done {
        let files = record::sealed(
            self.skill
                .files_with_record(&self.program.name, &self.program.version.to_string()),
        );
        // The assessment may be old: a person could have edited the skill, or
        // put one of their own in an empty place, since. What was there is
        // checked once it has been moved aside, so nothing changed before that
        // instant is overwritten.
        let mut changed = false;
        let result = replace_dir(&step.place, &step.skill_dir, &files, &mut |stage, path| {
            if stage == Stage::OldMovedAside && self.found(path) != step.found {
                changed = true;
                return Err(io::Error::other("changed since the assessment"));
            }
            Ok(())
        });
        match result {
            Ok(()) => done,
            Err(_) if changed => Done::Changed,
            Err(error) => Done::Failed(error.to_string()),
        }
    }
}

/// Where a replacement stands, for a check or a test that interrupts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The new version is written next to the place; nothing moved yet.
    Staged,
    /// The old version has been moved aside; the new one is not in place.
    OldMovedAside,
}

/// A sibling of the skills directory for staging, so no tool scanning the
/// skills directory ever sees a half-written skill as one of its own.
fn staging(place: &Path, skill_dir: &Path, kind: &str) -> io::Result<PathBuf> {
    let parent = place.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "a skills directory without a parent",
        )
    })?;
    let name = skill_dir.file_name().unwrap_or_default().to_string_lossy();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    Ok(parent.join(format!(".asi-{name}-{}-{stamp}.{kind}", std::process::id())))
}

/// Put `files` at `skill_dir`, replacing what is there in one step (D7).
///
/// The new version is written beside the skills directory and renamed into
/// place. A tool reading at the same moment finds the old version, the new
/// one, or for an instant none, never a mixture. On any failure the old
/// version is put back and the staging removed.
///
/// `interrupt` is called at each stage with the directory concerned: the
/// staged new version, then the old one moved aside, which no longer moves
/// under anyone's hands and is what a check should look at. An error there
/// stops the replacement.
pub fn replace_dir(
    place: &Path,
    skill_dir: &Path,
    files: &BTreeMap<PathBuf, Vec<u8>>,
    interrupt: &mut dyn FnMut(Stage, &Path) -> io::Result<()>,
) -> io::Result<()> {
    fs::create_dir_all(place)?;
    let new = staging(place, skill_dir, "new")?;
    let written = (|| {
        for (relative, content) in files {
            let path = new.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, content)?;
        }
        interrupt(Stage::Staged, &new)
    })();
    if let Err(error) = written {
        let _ = fs::remove_dir_all(&new);
        return Err(error);
    }

    let old = staging(place, skill_dir, "old")?;
    let had_old = fs::symlink_metadata(skill_dir).is_ok();
    if had_old && let Err(error) = fs::rename(skill_dir, &old) {
        let _ = fs::remove_dir_all(&new);
        return Err(error);
    }
    let moved = interrupt(Stage::OldMovedAside, &old).and_then(|()| fs::rename(&new, skill_dir));
    if let Err(error) = moved {
        if had_old {
            let _ = fs::rename(&old, skill_dir);
        }
        let _ = fs::remove_dir_all(&new);
        return Err(error);
    }
    if had_old {
        fs::remove_dir_all(&old)?;
    }
    Ok(())
}

/// Take a skill away in one step: moved aside first, then deleted, so a tool
/// never reads a half-deleted skill.
fn remove_dir(skill_dir: &Path) -> io::Result<()> {
    let place = skill_dir.parent().unwrap_or(skill_dir);
    let aside = staging(place, skill_dir, "gone")?;
    fs::rename(skill_dir, &aside)?;
    fs::remove_dir_all(aside)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(content: &str) -> BTreeMap<PathBuf, Vec<u8>> {
        BTreeMap::from([(PathBuf::from(SKILL_FILE), content.as_bytes().to_vec())])
    }

    fn leftovers(parent: &Path) -> Vec<String> {
        fs::read_dir(parent)
            .expect("read")
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|name| name.starts_with(".asi-"))
            .collect()
    }

    /// M3-A3: interrupted at either stage, the old version is intact and no
    /// staging is left behind.
    #[test]
    fn an_interrupted_replacement_leaves_the_old_skill_intact() {
        for stop_at in [Stage::Staged, Stage::OldMovedAside] {
            let home = tempfile::tempdir().expect("home");
            let place = home.path().join("tool/skills");
            let skill_dir = place.join("devserial");
            replace_dir(&place, &skill_dir, &files("old"), &mut |_, _| Ok(())).expect("first");

            let result = replace_dir(&place, &skill_dir, &files("new"), &mut |stage, _| {
                if stage == stop_at {
                    Err(io::Error::other("interrupted"))
                } else {
                    Ok(())
                }
            });

            assert!(result.is_err(), "{stop_at:?}");
            assert_eq!(
                fs::read_to_string(skill_dir.join(SKILL_FILE)).expect("old"),
                "old",
                "{stop_at:?}"
            );
            assert_eq!(
                leftovers(&home.path().join("tool")),
                Vec::<String>::new(),
                "{stop_at:?}"
            );
            assert_eq!(
                fs::read_dir(&place).expect("read").count(),
                1,
                "{stop_at:?}"
            );
        }
    }

    /// M3-A3, counter-probe: uninterrupted, the new version is in place and
    /// nothing is left over.
    #[test]
    fn an_uninterrupted_replacement_leaves_only_the_new_skill() {
        let home = tempfile::tempdir().expect("home");
        let place = home.path().join("tool/skills");
        let skill_dir = place.join("devserial");
        replace_dir(&place, &skill_dir, &files("old"), &mut |_, _| Ok(())).expect("first");
        replace_dir(&place, &skill_dir, &files("new"), &mut |_, _| Ok(())).expect("second");
        assert_eq!(
            fs::read_to_string(skill_dir.join(SKILL_FILE)).expect("new"),
            "new"
        );
        assert_eq!(leftovers(&home.path().join("tool")), Vec::<String>::new());
    }

    /// Staging happens beside the skills directory, never inside it, so a
    /// tool scanning the directory cannot pick up a half-written copy.
    #[test]
    fn staging_is_beside_the_skills_directory() {
        let home = tempfile::tempdir().expect("home");
        let place = home.path().join("tool/skills");
        let skill_dir = place.join("devserial");
        let mut seen = Vec::new();
        replace_dir(&place, &skill_dir, &files("x"), &mut |_, _| {
            seen.extend(
                fs::read_dir(&place)
                    .expect("read")
                    .filter_map(Result::ok)
                    .map(|e| e.file_name()),
            );
            Ok(())
        })
        .expect("replace");
        assert!(
            seen.is_empty(),
            "the skills directory held {seen:?} while staging"
        );
    }
}
