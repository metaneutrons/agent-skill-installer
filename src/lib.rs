// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

#![doc = include_str!("../README.md")]

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
