// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! Install an Agent Skill into the skill directories of AI coding tools.
//!
//! The crate is being planned; see `docs/plans/` in the repository. What
//! exists so far is the one rule every later step depends on: what a skill may
//! be called.

mod name;

pub use name::{NameError, validate_name};
