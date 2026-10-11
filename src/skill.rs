// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! The skill a program ships, held in memory until it is installed.
//!
//! The crate writes the frontmatter itself rather than editing YAML a program
//! hands over. That keeps the ownership record in one known shape, which is
//! what makes it readable again later without a YAML parser, and it rules out
//! a program's own `metadata` block colliding with the record.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::path::{Component, Path, PathBuf};

use crate::name::{NameError, validate_name};

/// The file every skill has, at the top of its directory.
pub const SKILL_FILE: &str = "SKILL.md";

/// The longest description the specification allows.
const MAX_DESCRIPTION: usize = 1024;

/// Frontmatter keys the crate writes itself.
const RESERVED_FIELDS: [&str; 3] = ["name", "description", "metadata"];

/// `metadata` keys that make up the ownership record.
pub const RECORD_KEYS: [&str; 3] = ["installed-by", "installed-version", "content-digest"];

/// A skill: its frontmatter, its instructions and any further files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    name: String,
    description: String,
    body: String,
    fields: Vec<(String, String)>,
    metadata: Vec<(String, String)>,
    files: BTreeMap<PathBuf, Vec<u8>>,
}

/// Why a skill was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SkillError {
    /// The name breaks the specification.
    Name(NameError),
    /// The description is empty or longer than 1024 characters.
    Description(usize),
    /// A frontmatter or metadata key is not a plain lowercase word, or is one
    /// the crate writes itself.
    Key(String),
    /// A file path is absolute, leaves the skill directory, or is `SKILL.md`.
    Path(PathBuf),
    /// A frontmatter key or metadata key given twice. YAML forbids it, and
    /// tools disagree on which value wins.
    Duplicate(String),
    /// A file path that differs from another, or from `SKILL.md`, only in
    /// letter case: one file on the case-insensitive file systems of macOS
    /// and Windows.
    CaseCollision(PathBuf),
}

impl fmt::Display for SkillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(error) => error.fmt(f),
            Self::Description(len) => write!(
                f,
                "a description has 1 to {MAX_DESCRIPTION} characters, this one has {len}"
            ),
            Self::Key(key) => write!(f, "'{key}' cannot be used as a frontmatter key here"),
            Self::Duplicate(key) => write!(f, "'{key}' is given twice"),
            Self::CaseCollision(path) => write!(
                f,
                "{} differs from another file only in letter case",
                path.display()
            ),
            Self::Path(path) => write!(
                f,
                "{} is not a relative path inside the skill",
                path.display()
            ),
        }
    }
}

impl std::error::Error for SkillError {}

impl From<NameError> for SkillError {
    fn from(error: NameError) -> Self {
        Self::Name(error)
    }
}

impl Skill {
    /// A skill with a name, a description and the instructions below the
    /// frontmatter.
    ///
    /// # Errors
    /// Returns an error for a name the specification refuses or a description
    /// that is empty or too long.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        body: impl Into<String>,
    ) -> Result<Self, SkillError> {
        let name = name.into();
        validate_name(&name)?;
        let description = description.into();
        let len = description.chars().count();
        if description.trim().is_empty() || len > MAX_DESCRIPTION {
            return Err(SkillError::Description(len));
        }
        Ok(Self {
            name,
            description,
            body: body.into(),
            fields: Vec::new(),
            metadata: Vec::new(),
            files: BTreeMap::new(),
        })
    }

    /// Add a top-level frontmatter field, such as `license`.
    ///
    /// # Errors
    /// Returns an error for a key that is not a plain lowercase word or that
    /// the crate writes itself.
    pub fn with_field(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, SkillError> {
        let key = key.into();
        if !plain_key(&key) || RESERVED_FIELDS.contains(&key.as_str()) {
            return Err(SkillError::Key(key));
        }
        if self.fields.iter().any(|(known, _)| *known == key) {
            return Err(SkillError::Duplicate(key));
        }
        self.fields.push((key, value.into()));
        Ok(self)
    }

    /// Add an entry to the `metadata` map, next to the ownership record.
    ///
    /// # Errors
    /// Returns an error for a key that is not a plain word or that belongs to
    /// the ownership record.
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, SkillError> {
        let key = key.into();
        // Tools namespace their own keys, as OpenCode's `opencode/slash` does.
        let plain = key.split('/').all(plain_key);
        if !plain || RECORD_KEYS.contains(&key.as_str()) {
            return Err(SkillError::Key(key));
        }
        if self.metadata.iter().any(|(known, _)| *known == key) {
            return Err(SkillError::Duplicate(key));
        }
        self.metadata.push((key, value.into()));
        Ok(self)
    }

    /// Add a further file, such as `references/api.md`.
    ///
    /// # Errors
    /// Returns an error for a path that is absolute, contains `..` or `.`,
    /// is empty, or names `SKILL.md` itself.
    pub fn with_file(
        mut self,
        path: impl Into<PathBuf>,
        content: impl Into<Vec<u8>>,
    ) -> Result<Self, SkillError> {
        let path = path.into();
        if !contained(&path) || path == Path::new(SKILL_FILE) {
            return Err(SkillError::Path(path));
        }
        let wanted = folded(&path);
        if wanted == folded_str(SKILL_FILE)
            || self.files.keys().any(|known| folded(known) == wanted)
        {
            return Err(SkillError::CaseCollision(path));
        }
        self.files.insert(path, content.into());
        Ok(self)
    }

    /// The skill's name, which is also its directory's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Every file of the skill with the record of who installed it, the
    /// digest left empty. [`crate::record`] fills it in.
    pub(crate) fn files_with_record(
        &self,
        program: &str,
        version: &str,
    ) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = self.files.clone();
        out.insert(
            PathBuf::from(SKILL_FILE),
            self.render(program, version).into_bytes(),
        );
        out
    }

    fn render(&self, program: &str, version: &str) -> String {
        // Writing into a String cannot fail.
        let mut text = String::from("---\n");
        let _ = writeln!(text, "name: {}", quote(&self.name));
        let _ = writeln!(text, "description: {}", quote(&self.description));
        for (key, value) in &self.fields {
            let _ = writeln!(text, "{key}: {}", quote(value));
        }
        text.push_str("metadata:\n");
        for (key, value) in &self.metadata {
            let _ = writeln!(text, "  {key}: {}", quote(value));
        }
        let _ = writeln!(text, "  installed-by: {}", quote(program));
        let _ = writeln!(text, "  installed-version: {}", quote(version));
        text.push_str("  content-digest: \"\"\n---\n");
        text.push_str(&self.body);
        if !self.body.ends_with('\n') {
            text.push('\n');
        }
        text
    }
}

/// A lowercase word with hyphens, the shape every frontmatter key in the
/// specification and the tools' documentation has.
fn plain_key(key: &str) -> bool {
    key.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A relative path that stays inside the directory it is joined to.
/// A path as a case-insensitive file system compares it, parts joined by
/// `/` whatever the system's separator.
fn folded(path: &Path) -> String {
    folded_str(
        &path
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn folded_str(text: &str) -> String {
    text.to_lowercase()
}

pub fn contained(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(name) if !name.is_empty()))
        // A backslash is a separator on Windows and a file name character
        // elsewhere; refused everywhere so the same skill means the same files.
        && !path.to_string_lossy().contains('\\')
}

/// A YAML double-quoted string.
///
/// Every value goes through this, so no value can end the frontmatter, start
/// a new key or carry a newline into the record.
pub fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Skill {
        Skill::new("devserial", "Serial terminal.", "body\n").expect("skill")
    }

    /// A key given twice is refused; YAML forbids it and tools disagree on
    /// which value wins.
    #[test]
    fn a_key_given_twice_is_refused() {
        let once = base().with_field("license", "MIT").expect("first");
        assert_eq!(
            once.with_field("license", "GPL-3.0").err(),
            Some(SkillError::Duplicate("license".into()))
        );
        let once = base()
            .with_metadata("opencode/slash", "true")
            .expect("first");
        assert_eq!(
            once.with_metadata("opencode/slash", "false").err(),
            Some(SkillError::Duplicate("opencode/slash".into()))
        );
    }

    /// Paths that differ only in letter case are one file on macOS and
    /// Windows, so the second is refused, and so is a `skill.md`.
    #[test]
    fn paths_differing_only_in_case_are_refused() {
        let one = base().with_file("references/cli.md", "a").expect("first");
        assert_eq!(
            one.clone().with_file("References/CLI.md", "b").err(),
            Some(SkillError::CaseCollision(PathBuf::from(
                "References/CLI.md"
            )))
        );
        assert_eq!(
            one.clone().with_file("skill.md", "c").err(),
            Some(SkillError::CaseCollision(PathBuf::from("skill.md")))
        );
        assert!(one.with_file("references/api.md", "d").is_ok());
    }
}
