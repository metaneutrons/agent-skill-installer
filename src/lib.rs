// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! Install an Agent Skill into the skill directories of AI coding tools.
//!
//! The design and its acceptance criteria are in `docs/plans/` in the
//! repository, the evidence about the tools in `docs/tools.md`.
//!
//! ```no_run
//! use agent_skill_installer::{
//!     Approval, Catalog, Consent, Environment, Installer, Program, Skill,
//! };
//!
//! let skill = Skill::new("devserial", "Serial terminal with a capture.", "Use `devserial read`.")?;
//! let program = Program::new("devserial", env!("CARGO_PKG_VERSION"))?;
//! let catalog = Catalog::builtin();
//! let env = Environment::from_process().expect("a home directory");
//! let installer = Installer::new(&catalog, &env, &skill, &program);
//!
//! // At every start: keep earlier installations current. Asks nobody.
//! installer.update()?;
//!
//! // When the user is asked: show what would happen, install what they agree to.
//! let mut consent = Consent::load("/path/in/your/data/dir/skill-consent")?;
//! let assessment = installer.assess(&consent)?;
//! let mut approval = Approval::default();
//! for step in assessment.awaiting_approval() {
//!     let yes = true; // ask in your own CLI, TUI or GUI
//!     if yes {
//!         approval.install.push(step.place.clone());
//!     } else {
//!         consent.decline(&step.place)?;
//!     }
//! }
//! consent.save()?;
//! installer.apply(&assessment, &approval);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod catalog;
mod consent;
mod env;
mod install;
mod name;
mod plan;
mod record;
mod resolve;
mod skill;

pub use catalog::{Catalog, CatalogError, Location, SkillDir, Tool};
pub use consent::Consent;
pub use env::Environment;
pub use install::{
    Action, Approval, Assessment, Done, Found, Installer, Outcome, Problem, Program, Step, Why,
};
pub use name::{NameError, validate_name};
pub use plan::{Plan, PlanError, Target, Unserved, existing_places, plan};
pub use record::Record;
pub use skill::{SKILL_FILE, Skill, SkillError};
