// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! The everyday way to use the crate: one value that knows the skill, the
//! program, the machine and where the user's answers are kept.
//!
//! [`Installer`] and [`Approval`] stay available for a program that wants to
//! decide place by place; this is a thin layer over them.

use std::fmt;
use std::io;
use std::path::PathBuf;

use crate::catalog::Catalog;
use crate::consent::Consent;
use crate::env::Environment;
use crate::install::{Action, Approval, Assessment, Installer, Outcome, Program, Step, Why};
use crate::plan::PlanError;
use crate::skill::Skill;

/// Why the machine could not be looked at or the answers not be kept.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// No home directory to look for AI tools in.
    NoHome,
    /// No plan could be made.
    Plan(PlanError),
    /// The user's answers could not be read or written.
    Consent(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHome => f.write_str("no home directory to look for AI tools in"),
            Self::Plan(error) => error.fmt(f),
            Self::Consent(error) => write!(f, "the answers about the skill: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoHome | Self::Plan(_) => None,
            Self::Consent(error) => Some(error),
        }
    }
}

impl From<PlanError> for Error {
    fn from(error: PlanError) -> Self {
        Self::Plan(error)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Consent(error)
    }
}

/// A program's skill on the user's machine.
///
/// ```no_run
/// use agent_skill_installer::{AgentSkill, Program, Skill};
///
/// let skill = Skill::new("mytool", "Use mytool to do the thing.", "# mytool\n")?;
/// let program = Program::new("mytool", "1.2.0")?;
/// let agent = AgentSkill::new(skill, program, "/data/mytool/skill-consent")?;
///
/// agent.update()?; // at start, without asking
/// if let Some(offer) = agent.offer()? {
///     // Ask once in your own surface, naming offer.steps().
///     offer.accept()?; // or offer.decline()?
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct AgentSkill {
    catalog: Catalog,
    env: Environment,
    skill: Skill,
    program: Program,
    consent_file: PathBuf,
}

impl AgentSkill {
    /// The skill for the user running the program, with the built-in tools.
    ///
    /// `consent_file` is where the places the user declined are kept; the
    /// program's own data directory is the natural home for it.
    ///
    /// # Errors
    /// Returns [`Error::NoHome`] when the process has no home directory.
    pub fn new(
        skill: Skill,
        program: Program,
        consent_file: impl Into<PathBuf>,
    ) -> Result<Self, Error> {
        Ok(Self {
            catalog: Catalog::builtin(),
            env: Environment::from_process().ok_or(Error::NoHome)?,
            skill,
            program,
            consent_file: consent_file.into(),
        })
    }

    /// The same, looking at another environment: a test's temporary home.
    #[must_use]
    pub fn with_environment(mut self, env: Environment) -> Self {
        self.env = env;
        self
    }

    /// The same, with a catalog the program extended or replaced.
    #[must_use]
    pub fn with_catalog(mut self, catalog: Catalog) -> Self {
        self.catalog = catalog;
        self
    }

    /// The tools and folders it knows, for naming them to people.
    #[must_use]
    pub const fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// The installer underneath, for a program that decides place by place.
    #[must_use]
    pub const fn installer(&self) -> Installer<'_> {
        Installer::new(&self.catalog, &self.env, &self.skill, &self.program)
    }

    /// Every place and what would happen there. Writes nothing.
    ///
    /// # Errors
    /// Returns an error when the answers cannot be read or no plan can be
    /// made.
    pub fn status(&self) -> Result<Assessment, Error> {
        Ok(self.installer().assess(&self.consent()?)?)
    }

    /// Bring every copy this program installed and nobody changed up to the
    /// running version. Asks nothing and installs nothing new; meant for every
    /// start of the program.
    ///
    /// # Errors
    /// Returns an error when no plan can be made.
    pub fn update(&self) -> Result<Vec<Outcome>, Error> {
        Ok(self.installer().update()?)
    }

    /// The question to ask, when a place exists nobody has decided on.
    ///
    /// # Errors
    /// Returns an error when the answers cannot be read or no plan can be
    /// made.
    pub fn offer(&self) -> Result<Option<Offer<'_>>, Error> {
        let assessment = self.status()?;
        let steps: Vec<Step> = assessment.awaiting_approval().cloned().collect();
        Ok((!steps.is_empty()).then_some(Offer { agent: self, steps }))
    }

    /// Install wherever the skill can go, also where the user declined
    /// before: for an explicit request such as an `install` command.
    ///
    /// # Errors
    /// Returns an error when the answers cannot be read or written, or no
    /// plan can be made.
    pub fn install_everywhere(&self) -> Result<Vec<Outcome>, Error> {
        let assessment = self.status()?;
        let places: Vec<PathBuf> = assessment
            .steps
            .iter()
            .filter(|step| {
                matches!(
                    step.action,
                    Action::Install | Action::Nothing(Why::Declined)
                )
            })
            .map(|step| step.place.clone())
            .collect();
        self.install(places)
    }

    /// Remove every copy this program installed and nobody changed.
    ///
    /// # Errors
    /// Returns an error when no plan can be made.
    pub fn remove(&self) -> Result<Vec<Outcome>, Error> {
        Ok(self.installer().remove()?)
    }

    fn consent(&self) -> io::Result<Consent> {
        Consent::load(&self.consent_file)
    }

    /// Install into `places`, which the user agreed to, withdrawing any
    /// earlier decline there. Also brings older unchanged copies up to date.
    fn install(&self, places: Vec<PathBuf>) -> Result<Vec<Outcome>, Error> {
        let mut consent = self.consent()?;
        for place in &places {
            consent.withdraw(place);
        }
        consent.save()?;
        let assessment = self.installer().assess(&consent)?;
        Ok(self
            .installer()
            .apply(&assessment, &Approval::install(places)))
    }
}

/// Places nobody has decided on, to be put to the user once.
#[derive(Debug)]
pub struct Offer<'a> {
    agent: &'a AgentSkill,
    steps: Vec<Step>,
}

impl Offer<'_> {
    /// The places, with the tools that read each, to name in the question.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// The user agreed: install into every offered place.
    ///
    /// # Errors
    /// Returns an error when the answers cannot be read or written, or no
    /// plan can be made.
    pub fn accept(self) -> Result<Vec<Outcome>, Error> {
        self.agent
            .install(self.steps.into_iter().map(|step| step.place).collect())
    }

    /// The user said no: never offer these places again. A tool installed
    /// later is offered on its own.
    ///
    /// # Errors
    /// Returns an error when the answers cannot be read or written.
    pub fn decline(self) -> Result<(), Error> {
        let mut consent = self.agent.consent()?;
        for step in &self.steps {
            consent.decline(&step.place)?;
        }
        Ok(consent.save()?)
    }
}
