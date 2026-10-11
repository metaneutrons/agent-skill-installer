// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! Who installed a skill, and whether anybody changed it since (D4).
//!
//! The record lives in the skill's own frontmatter, in the `metadata` map the
//! specification provides. `content-digest` covers every file of the skill
//! directory with that one value blanked, so the digest can sit inside what it
//! covers.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use crate::skill::SKILL_FILE;

/// Files the operating system drops into directories a person merely looked
/// at. They count neither as content nor as a change, or one glance in a file
/// manager would freeze a skill for good.
const IGNORED: [&str; 2] = [".DS_Store", "Thumbs.db"];

/// What a skill's frontmatter says about where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// The program that installed it.
    pub installed_by: String,
    /// The program's version at the time.
    pub installed_version: String,
    /// The digest written at installation.
    pub content_digest: String,
}

/// The line the digest occupies, in the shape the crate writes it.
const DIGEST_KEY: &str = "  content-digest: ";

/// Blank the digest's value, so a skill hashes the same before and after the
/// digest is written into it.
fn blanked(skill_md: &[u8]) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(skill_md) else {
        return skill_md.to_vec();
    };
    let mut out = String::with_capacity(text.len());
    let mut in_frontmatter = false;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let bare = line.trim_end_matches(['\n', '\r']);
        if bare == "---" {
            in_frontmatter = index == 0;
        }
        if in_frontmatter && bare.starts_with(DIGEST_KEY) {
            out.push_str(DIGEST_KEY);
            out.push_str("\"\"");
            out.push_str(&line[bare.len()..]);
        } else {
            out.push_str(line);
        }
    }
    out.into_bytes()
}

/// The digest of a set of files, the digest line blanked.
///
/// Each file contributes its path, a zero byte, its length and its content,
/// in path order, so neither moving bytes between files nor renaming one
/// leaves the digest unchanged.
pub fn digest(files: &BTreeMap<PathBuf, Vec<u8>>) -> String {
    let mut hasher = Sha256::new();
    for (path, content) in files {
        let content = if path == Path::new(SKILL_FILE) {
            blanked(content)
        } else {
            content.clone()
        };
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update((content.len() as u64).to_be_bytes());
        hasher.update(&content);
    }
    let hex = hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            // Writing into a String cannot fail.
            let _ = write!(hex, "{byte:02x}");
            hex
        });
    format!("sha256:{hex}")
}

/// The files with the digest written into `SKILL.md`.
pub fn sealed(mut files: BTreeMap<PathBuf, Vec<u8>>) -> BTreeMap<PathBuf, Vec<u8>> {
    let value = digest(&files);
    if let Some(skill_md) = files.get_mut(Path::new(SKILL_FILE)) {
        let text = String::from_utf8_lossy(skill_md).into_owned();
        let line = format!("{DIGEST_KEY}\"\"\n");
        let sealed_line = format!("{DIGEST_KEY}\"{value}\"\n");
        *skill_md = text.replacen(&line, &sealed_line, 1).into_bytes();
    }
    files
}

/// Read every file below a skill directory.
///
/// # Errors
/// Returns an error when the directory cannot be read, or when it contains
/// something other than files and directories: a symlink inside a skill
/// could make a later write leave it.
pub fn read_dir(root: &Path) -> io::Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        for entry in fs::read_dir(root.join(&relative))? {
            let entry = entry?;
            let name = entry.file_name();
            if IGNORED.iter().any(|ignored| name == *ignored) {
                continue;
            }
            let path = relative.join(&name);
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() {
                files.insert(path, fs::read(entry.path())?);
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{} is neither a file nor a directory", path.display()),
                ));
            }
        }
    }
    Ok(files)
}

/// The record in a `SKILL.md`, if it has one in the shape the crate writes.
///
/// Anything else, including a record somebody rewrote in another YAML style,
/// reads as no record: the crate then cannot show the skill is its own and
/// leaves it alone, which is the safe way to be wrong.
pub fn parse(skill_md: &[u8]) -> Option<Record> {
    let text = std::str::from_utf8(skill_md).ok()?;
    let mut lines = text.lines();
    if lines.next()? != "---" {
        return None;
    }
    let mut in_metadata = false;
    let mut values = BTreeMap::new();
    for line in lines {
        if line == "---" {
            break;
        }
        if !line.starts_with(' ') {
            in_metadata = line == "metadata:";
            continue;
        }
        if in_metadata && let Some((key, value)) = line.trim_start().split_once(": ") {
            values.insert(key.to_string(), unquote(value)?);
        }
    }
    Some(Record {
        installed_by: values.remove("installed-by")?,
        installed_version: values.remove("installed-version")?,
        content_digest: values.remove("content-digest")?,
    })
}

/// The inverse of [`crate::skill::quote`].
fn unquote(value: &str) -> Option<String> {
    let inner = value.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'u' => {
                let code: String = chars.by_ref().take(4).collect();
                out.push(char::from_u32(u32::from_str_radix(&code, 16).ok()?)?);
            }
            _ => return None,
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(nested: PathBuf) -> BTreeMap<PathBuf, Vec<u8>> {
        BTreeMap::from([
            (
                PathBuf::from(SKILL_FILE),
                b"---\nname: x\n---\nbody\n".to_vec(),
            ),
            (nested, b"# CLI\n".to_vec()),
        ])
    }

    /// A path as a program writes it and as it is read back from disk give
    /// the same digest, and that digest is the same on every system.
    #[test]
    fn the_digest_does_not_depend_on_the_path_separator() {
        let given = digest(&files(PathBuf::from("references/cli.md")));
        let read_back = digest(&files(Path::new("references").join("cli.md")));
        assert_eq!(given, read_back);
        assert_eq!(given, GOLDEN);
    }

    /// Computed once; a change here breaks every installed record.
    const GOLDEN: &str = "sha256:6cc04206c47535a97a74d522b2dcc77d36d66e7cae2f3ddcae4f949e6bd5d252";
}
