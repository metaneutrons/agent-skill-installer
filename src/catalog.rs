// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! Which tools read skills from where.
//!
//! The built-in entries follow `docs/tools.md`: the mainstream tools only, each
//! with the directories its vendor documents. A program can add its own tools
//! and directories; the planner treats them exactly like the built-in ones.

use std::path::{Path, PathBuf};

use crate::env::Environment;

/// Where something lives: below the home directory, unless a variable moves
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    home_relative: PathBuf,
    relocated_by: Option<(String, PathBuf)>,
}

impl Location {
    /// A path relative to the home directory.
    #[must_use]
    pub fn home(relative: impl Into<PathBuf>) -> Self {
        Self {
            home_relative: relative.into(),
            relocated_by: None,
        }
    }

    /// The same location, moved to `$var/suffix` when `var` is set.
    #[must_use]
    pub fn or_env(mut self, var: impl Into<String>, suffix: impl Into<PathBuf>) -> Self {
        self.relocated_by = Some((var.into(), suffix.into()));
        self
    }

    /// The path this location has in an environment.
    #[must_use]
    pub fn resolve(&self, env: &Environment) -> PathBuf {
        if let Some((var, suffix)) = &self.relocated_by
            && let Some(value) = env.var(var)
        {
            // An empty suffix means the variable names the place itself;
            // joining "" would only append a separator.
            return if suffix.as_os_str().is_empty() {
                PathBuf::from(value)
            } else {
                Path::new(value).join(suffix)
            };
        }
        env.home().join(&self.home_relative)
    }
}

/// A directory some tool reads skills from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SkillDir {
    /// A short, stable identifier, used by [`Tool`] to refer to it.
    pub id: String,
    /// Where the directory is.
    pub path: Location,
    /// The tool home that has to exist before this directory may be created.
    ///
    /// `None` for a vendor-neutral directory, which belongs to no single tool
    /// and may be created whenever a tool that reads it is installed.
    pub requires: Option<Location>,
    /// Preferred when two choices are otherwise equal. Set for the
    /// vendor-neutral directory, because more tools read it.
    pub preferred: bool,
}

impl SkillDir {
    /// A directory at `path`, owned by no single tool and not preferred.
    #[must_use]
    pub fn new(id: impl Into<String>, path: Location) -> Self {
        Self {
            id: id.into(),
            path,
            requires: None,
            preferred: false,
        }
    }

    /// The same directory, created only once `home` exists.
    #[must_use]
    pub fn requires(mut self, home: Location) -> Self {
        self.requires = Some(home);
        self
    }

    /// The same directory, preferred when two choices are otherwise equal.
    #[must_use]
    pub const fn preferred(mut self) -> Self {
        self.preferred = true;
        self
    }
}

/// A tool that reads skills.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Tool {
    /// A short, stable identifier.
    pub id: String,
    /// The name a person knows the tool by.
    pub name: String,
    /// The directory whose existence shows that the tool is installed.
    pub home: Location,
    /// Directories the tool reads in every installation. A skill in one of
    /// them reaches the tool.
    pub reads: Vec<String>,
    /// Directories only some installations read, such as an editor
    /// integration reading more than the command line does. They never count
    /// as reaching the tool, but a copy there may still be seen twice.
    pub may_read: Vec<String>,
}

impl Tool {
    /// A tool, installed when `home` exists, that reads no directory yet.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, home: Location) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            home,
            reads: Vec::new(),
            may_read: Vec::new(),
        }
    }

    /// The same tool, reading these directories in every installation.
    #[must_use]
    pub fn reads<I, S>(mut self, dirs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.reads.extend(dirs.into_iter().map(Into::into));
        self
    }

    /// The same tool, reading these directories only in some installations.
    #[must_use]
    pub fn may_read<I, S>(mut self, dirs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.may_read.extend(dirs.into_iter().map(Into::into));
        self
    }
}

/// The tools and directories the planner knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    pub(crate) dirs: Vec<SkillDir>,
    pub(crate) tools: Vec<Tool>,
}

/// Why an entry could not be added to a catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CatalogError {
    /// An entry with this identifier is already there.
    DuplicateId(String),
    /// A tool refers to a directory the catalog does not have.
    UnknownDir {
        /// The tool that refers to it.
        tool: String,
        /// The identifier that is not in the catalog.
        dir: String,
    },
    /// A tool reads no directory, so no skill could ever reach it.
    ReadsNothing(String),
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "the catalog already has an entry '{id}'"),
            Self::UnknownDir { tool, dir } => {
                write!(
                    f,
                    "tool '{tool}' reads '{dir}', which the catalog does not have"
                )
            }
            Self::ReadsNothing(tool) => write!(f, "tool '{tool}' reads no directory"),
        }
    }
}

impl std::error::Error for CatalogError {}

fn dir(id: &str, path: Location, requires: Option<Location>, preferred: bool) -> SkillDir {
    SkillDir {
        id: id.to_string(),
        path,
        requires,
        preferred,
    }
}

fn tool(id: &str, name: &str, home: Location, reads: &[&str], may_read: &[&str]) -> Tool {
    Tool {
        id: id.to_string(),
        name: name.to_string(),
        home,
        reads: reads.iter().map(ToString::to_string).collect(),
        may_read: may_read.iter().map(ToString::to_string).collect(),
    }
}

impl Catalog {
    /// An empty catalog, for a program that wants none of the built-in
    /// entries.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            dirs: Vec::new(),
            tools: Vec::new(),
        }
    }

    /// The mainstream tools, as documented in `docs/tools.md`.
    #[must_use]
    pub fn builtin() -> Self {
        let claude_home = Location::home(".claude").or_env("CLAUDE_CONFIG_DIR", "");
        let copilot_home = Location::home(".copilot").or_env("COPILOT_HOME", "");
        // From secondary sources only, see docs/tools.md.
        let opencode_home =
            Location::home(".config/opencode").or_env("XDG_CONFIG_HOME", "opencode");

        let dirs = vec![
            dir(
                "claude",
                Location::home(".claude/skills").or_env("CLAUDE_CONFIG_DIR", "skills"),
                Some(claude_home.clone()),
                false,
            ),
            dir("agents", Location::home(".agents/skills"), None, true),
            dir(
                "gemini",
                Location::home(".gemini/skills"),
                Some(Location::home(".gemini")),
                false,
            ),
            dir(
                "copilot",
                Location::home(".copilot/skills").or_env("COPILOT_HOME", "skills"),
                Some(copilot_home.clone()),
                false,
            ),
            dir(
                "cursor",
                Location::home(".cursor/skills"),
                Some(Location::home(".cursor")),
                false,
            ),
            dir(
                "opencode",
                Location::home(".config/opencode/skills")
                    .or_env("XDG_CONFIG_HOME", "opencode/skills"),
                Some(opencode_home.clone()),
                false,
            ),
        ];

        let tools = vec![
            tool("claude-code", "Claude Code", claude_home, &["claude"], &[]),
            tool(
                "codex",
                "Codex",
                Location::home(".codex").or_env("CODEX_HOME", ""),
                &["agents"],
                &[],
            ),
            tool(
                "gemini-cli",
                "Gemini CLI",
                Location::home(".gemini"),
                &["gemini", "agents"],
                &[],
            ),
            // The command line reads two directories; Copilot in VS Code reads
            // `~/.claude/skills` as well, and shares the home directory.
            tool(
                "copilot",
                "GitHub Copilot",
                copilot_home,
                &["copilot", "agents"],
                &["claude"],
            ),
            tool(
                "cursor",
                "Cursor",
                Location::home(".cursor"),
                &["cursor", "agents", "claude"],
                &[],
            ),
            tool(
                "opencode",
                "OpenCode",
                opencode_home,
                &["opencode", "claude", "agents"],
                &[],
            ),
        ];

        Self { dirs, tools }
    }

    /// Add a directory.
    ///
    /// # Errors
    /// Returns an error when the identifier is taken.
    pub fn add_dir(&mut self, dir: SkillDir) -> Result<&mut Self, CatalogError> {
        if self.dirs.iter().any(|known| known.id == dir.id) {
            return Err(CatalogError::DuplicateId(dir.id));
        }
        self.dirs.push(dir);
        Ok(self)
    }

    /// Add a tool.
    ///
    /// # Errors
    /// Returns an error when the identifier is taken, the tool reads nothing,
    /// or it refers to a directory the catalog does not have.
    pub fn add_tool(&mut self, tool: Tool) -> Result<&mut Self, CatalogError> {
        if self.tools.iter().any(|known| known.id == tool.id) {
            return Err(CatalogError::DuplicateId(tool.id));
        }
        if tool.reads.is_empty() {
            return Err(CatalogError::ReadsNothing(tool.id));
        }
        if let Some(unknown) = tool
            .reads
            .iter()
            .chain(&tool.may_read)
            .find(|id| !self.dirs.iter().any(|dir| &dir.id == *id))
        {
            return Err(CatalogError::UnknownDir {
                tool: tool.id.clone(),
                dir: unknown.clone(),
            });
        }
        self.tools.push(tool);
        Ok(self)
    }

    /// The tools in this catalog.
    #[must_use]
    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    /// The directories in this catalog.
    #[must_use]
    pub fn dirs(&self) -> &[SkillDir] {
        &self.dirs
    }

    pub(crate) fn dir_index(&self, id: &str) -> Option<usize> {
        self.dirs.iter().position(|dir| dir.id == id)
    }
}
