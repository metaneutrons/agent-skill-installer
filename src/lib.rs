// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

#![doc = include_str!("../README.md")]

mod agent;
mod catalog;
mod consent;
mod env;
mod install;
mod name;
mod plan;
mod record;
mod resolve;
mod skill;

pub use agent::{AgentSkill, Error, Offer};
pub use catalog::{Catalog, CatalogError, Location, SkillDir, Tool};
pub use consent::Consent;
pub use env::Environment;
pub use install::{
    Action, Approval, Assessment, Done, Found, Installer, Outcome, Problem, Program, Step, Why,
};
pub use name::{NameError, validate_name};
pub use plan::{Plan, PlanError, Target, Unserved, plan};
pub use skill::{SKILL_FILE, Skill, SkillError};

/// The version type [`Program`] and [`Found`] use, so a caller needs no
/// dependency of its own that could drift from this one.
pub use semver;
