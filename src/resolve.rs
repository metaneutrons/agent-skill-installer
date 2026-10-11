// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! Where a path really leads, and whether that place is version controlled.

use std::io;
use std::path::{Path, PathBuf};

/// The physical place a path leads to, whether or not it exists yet.
///
/// The longest existing ancestor is resolved through every symlink and the
/// part that does not exist yet is appended unchanged. That is the place a
/// directory created at `path` would end up, which is what deduplication
/// needs: `~/.gemini/skills` as a symlink to `~/.claude/skills` is one place,
/// and so is a not yet existing `~/.gemini/skills/x` below it.
///
/// # Errors
/// Returns an error when an existing component cannot be resolved, such as a
/// dangling symlink or a loop. Writing through such a path would go somewhere
/// nobody can predict.
pub fn physical(path: &Path) -> io::Result<PathBuf> {
    let mut missing = Vec::new();
    let mut current = path;
    loop {
        match std::fs::canonicalize(current) {
            Ok(resolved) => {
                let resolved = simplified(resolved);
                return Ok(missing
                    .iter()
                    .rev()
                    .fold(resolved, |acc, part: &&std::ffi::OsStr| acc.join(part)));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // A symlink whose target is missing reports NotFound too, but
                // it exists as a link; going past it would hide that.
                if std::fs::symlink_metadata(current).is_ok() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("{} is a dangling symlink", current.display()),
                    ));
                }
                let (Some(parent), Some(name)) = (current.parent(), current.file_name()) else {
                    return Err(error);
                };
                missing.push(name);
                current = parent;
            }
            Err(error) => return Err(error),
        }
    }
}

/// A canonical path as people and the other tools write it.
///
/// Windows hands canonical paths out in the verbatim form `\\?\C:\...`.
/// Every tool, and a consent file a user may read, writes `C:\...`; a place
/// compared or shown in the long form would look like another one. Only a
/// plain drive path is shortened; a verbatim UNC path stays as it is.
#[cfg(windows)]
fn simplified(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\")
        && rest.as_bytes().get(1) == Some(&b':')
        && rest.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
    {
        return PathBuf::from(rest);
    }
    path
}

/// Elsewhere canonical paths are already the ones people write.
#[cfg(not(windows))]
const fn simplified(path: PathBuf) -> PathBuf {
    path
}

/// The root of the git working tree a place lies in, if any.
///
/// Presence of `.git`, a directory in a normal clone and a file in a linked
/// worktree, is enough. Nothing is run and nothing in the repository is read.
#[must_use]
pub fn git_root(physical: &Path) -> Option<PathBuf> {
    physical
        .ancestors()
        .find(|dir| dir.join(".git").symlink_metadata().is_ok())
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On Windows a canonical path comes back as `C:\\...`, as people and
    /// tools write it, not in the verbatim `\\\\?\\` form.
    #[cfg(windows)]
    #[test]
    fn a_physical_path_is_written_as_people_write_it() {
        let dir = tempfile::tempdir().expect("dir");
        let place = physical(&dir.path().join("skills")).expect("physical");
        assert!(
            !place.to_string_lossy().starts_with(r"\\?\"),
            "{}",
            place.display()
        );
    }

    /// Elsewhere the path is the canonical one, unchanged.
    #[test]
    fn a_physical_path_resolves_symlinks_only() {
        let dir = tempfile::tempdir().expect("dir");
        let resolved = std::fs::canonicalize(dir.path()).expect("canonical");
        assert_eq!(
            physical(&dir.path().join("skills")).expect("physical"),
            resolved.join("skills")
        );
    }
}
