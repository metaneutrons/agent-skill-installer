// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! Which targets the user said no to (D5).
//!
//! Kept by the calling program, in a file in its own data directory. Nothing
//! about consent is written into the tools' directories: an installed skill is
//! itself the record that the user said yes, and a "no" belongs to the program
//! that asked.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const HEADER: &str = "# agent-skill-installer: targets the user declined, one per line";

/// The declined targets, and the file they are kept in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consent {
    path: PathBuf,
    declined: BTreeSet<PathBuf>,
}

impl Consent {
    /// The store kept at `path`. A file that does not exist yet is an empty
    /// store.
    ///
    /// # Errors
    /// Returns an error when the file exists but cannot be read.
    pub fn load(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let declined = match fs::read_to_string(&path) {
            Ok(text) => text
                .lines()
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(PathBuf::from)
                .collect(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeSet::new(),
            Err(error) => return Err(error),
        };
        Ok(Self { path, declined })
    }

    /// Whether the user declined this target.
    #[must_use]
    pub fn is_declined(&self, target: &Path) -> bool {
        self.declined.contains(target)
    }

    /// Record that the user declined this target.
    ///
    /// # Errors
    /// Returns an error for a path that cannot be stored on one line.
    pub fn decline(&mut self, target: &Path) -> io::Result<()> {
        let text = target.to_string_lossy();
        if text.contains('\n') || text.contains('\r') || text.as_ref() != target.as_os_str() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} cannot be stored as one line of text", target.display()),
            ));
        }
        self.declined.insert(target.to_path_buf());
        Ok(())
    }

    /// Forget that the user declined this target, so it is asked about again.
    pub fn withdraw(&mut self, target: &Path) {
        self.declined.remove(target);
    }

    /// Write the store, replacing the file in one step.
    ///
    /// # Errors
    /// Returns an error when the file or its directory cannot be written.
    pub fn save(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut text = format!("{HEADER}\n");
        for target in &self.declined {
            text.push_str(&target.to_string_lossy());
            text.push('\n');
        }
        let staging = self.path.with_extension("tmp");
        let mut file = fs::File::create(&staging)?;
        std::io::Write::write_all(&mut file, text.as_bytes())?;
        // On disk before the rename, so a crash leaves the old answers or the
        // new ones, never an empty file.
        file.sync_all()?;
        drop(file);
        fs::rename(&staging, &self.path)
    }
}
