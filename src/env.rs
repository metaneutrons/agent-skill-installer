// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! The parts of the process environment the planner depends on.
//!
//! Passed in rather than read inside, so a test can describe a whole machine
//! in a temporary directory and a program can plan for a home other than its
//! own.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// A home directory and the environment variables that relocate tools.
#[derive(Clone, PartialEq, Eq)]
pub struct Environment {
    home: PathBuf,
    vars: BTreeMap<String, OsString>,
}

/// Names only: the process environment holds tokens and passwords, and a
/// program that logs its environment must not log those.
impl std::fmt::Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Environment")
            .field("home", &self.home)
            .field("vars", &self.vars.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Environment {
    /// An environment with the given home and no variables set.
    #[must_use]
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self {
            home: home.into(),
            vars: BTreeMap::new(),
        }
    }

    /// The same environment with one variable set.
    #[must_use]
    pub fn with_var(mut self, name: impl Into<String>, value: impl Into<OsString>) -> Self {
        self.vars.insert(name.into(), value.into());
        self
    }

    /// The environment of the running process.
    ///
    /// Returns `None` when no home directory can be found, because every
    /// target lives below it and guessing one would write somewhere unknown.
    #[must_use]
    pub fn from_process() -> Option<Self> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .filter(|value| !value.is_empty())?;
        let vars = std::env::vars_os()
            .filter_map(|(name, value)| Some((name.into_string().ok()?, value)))
            .collect();
        Some(Self {
            home: PathBuf::from(home),
            vars,
        })
    }

    /// The home directory.
    #[must_use]
    pub fn home(&self) -> &Path {
        &self.home
    }

    /// A variable, when it is set to something other than the empty string.
    ///
    /// An empty value is treated as unset, as most tools do: `CLAUDE_CONFIG_DIR=`
    /// does not move anything to the current directory.
    #[must_use]
    pub fn var(&self, name: &str) -> Option<&OsString> {
        self.vars.get(name).filter(|value| !value.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The process environment holds secrets; Debug shows only names.
    #[test]
    fn debug_shows_variable_names_not_values() {
        let env = Environment::new("/home/x").with_var("API_TOKEN", "s3cr3t-value");
        let shown = format!("{env:?}");
        assert!(shown.contains("API_TOKEN"), "{shown}");
        assert!(!shown.contains("s3cr3t-value"), "{shown}");
    }
}
