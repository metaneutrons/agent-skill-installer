// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! Install an Agent Skill into the skill directories of AI coding tools.
//!
//! The design and its acceptance criteria are in `docs/plans/` in the
//! repository, the evidence about the tools in `docs/tools.md`. What exists so
//! far: what a skill may be called, and where on a machine it should go.
//!
//! ```no_run
//! use agent_skill_installer::{Catalog, Environment, plan};
//!
//! let env = Environment::from_process().expect("a home directory");
//! let plan = plan(&Catalog::builtin(), &env).expect("a plan");
//! for target in &plan.targets {
//!     println!("{} for {}", target.path.display(), target.tools.join(", "));
//! }
//! ```

mod catalog;
mod env;
mod name;
mod plan;
mod resolve;

pub use catalog::{Catalog, CatalogError, Location, SkillDir, Tool};
pub use env::Environment;
pub use name::{NameError, validate_name};
pub use plan::{Plan, PlanError, Target, Unserved, plan};
