// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! What a skill may be called.
//!
//! The Agent Skills specification allows 1 to 64 characters of lowercase
//! ASCII letters, digits and hyphens, with no hyphen at either end and no two
//! in a row, and requires the name to match the skill's directory. The
//! strictest tools enforce exactly that: VS Code's Copilot drops a skill with
//! an invalid name without a word, and `OpenCode` matches
//! `^[a-z0-9]+(-[a-z0-9]+)*$`. A name that passes here is a name every
//! supported tool loads.
//!
//! The name also becomes a directory, which is the second reason to be strict:
//! nothing that passes can contain a path separator, `..` or a character a
//! file system treats specially.

use std::fmt;

/// The longest name the specification allows.
const MAX_LEN: usize = 64;

/// Why a skill name was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameError {
    /// The name is empty.
    Empty,
    /// The name is longer than the specification allows.
    TooLong(usize),
    /// The name contains a character other than `a-z`, `0-9` and `-`.
    InvalidChar(char),
    /// The name starts or ends with a hyphen.
    EdgeHyphen,
    /// The name contains two hyphens in a row.
    DoubleHyphen,
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("a skill name cannot be empty"),
            Self::TooLong(len) => {
                write!(
                    f,
                    "a skill name has at most {MAX_LEN} characters, this one has {len}"
                )
            }
            Self::InvalidChar(c) => {
                write!(
                    f,
                    "a skill name may only contain a-z, 0-9 and '-', not {c:?}"
                )
            }
            Self::EdgeHyphen => f.write_str("a skill name cannot start or end with '-'"),
            Self::DoubleHyphen => f.write_str("a skill name cannot contain '--'"),
        }
    }
}

impl std::error::Error for NameError {}

/// Check a skill name against the Agent Skills specification.
///
/// # Errors
/// Returns the first rule the name breaks.
pub fn validate_name(name: &str) -> Result<(), NameError> {
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    // Counted in characters, which for a valid name are bytes; an invalid one
    // is refused below whatever its length.
    let len = name.chars().count();
    if len > MAX_LEN {
        return Err(NameError::TooLong(len));
    }
    if let Some(c) = name
        .chars()
        .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-'))
    {
        return Err(NameError::InvalidChar(c));
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err(NameError::EdgeHyphen);
    }
    if name.contains("--") {
        return Err(NameError::DoubleHyphen);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_specification_allows_pass() {
        let longest = "a".repeat(64);
        for name in [
            "devserial",
            "devknx",
            "a",
            "pdf-processing",
            "x2",
            longest.as_str(),
        ] {
            assert_eq!(validate_name(name), Ok(()), "{name}");
        }
    }

    #[test]
    fn each_rule_refuses_for_its_own_reason() {
        assert_eq!(validate_name(""), Err(NameError::Empty));
        assert_eq!(validate_name(&"a".repeat(65)), Err(NameError::TooLong(65)));
        assert_eq!(validate_name("DevSerial"), Err(NameError::InvalidChar('D')));
        assert_eq!(
            validate_name("dev_serial"),
            Err(NameError::InvalidChar('_'))
        );
        assert_eq!(validate_name("-dev"), Err(NameError::EdgeHyphen));
        assert_eq!(validate_name("dev-"), Err(NameError::EdgeHyphen));
        assert_eq!(validate_name("dev--serial"), Err(NameError::DoubleHyphen));
    }

    /// A name becomes a directory, so nothing that could leave it may pass.
    #[test]
    fn nothing_that_could_leave_the_directory_passes() {
        for name in [
            "..", "../x", "a/b", "a\\b", ".hidden", "a.b", "ä", "a b", "a\0b",
        ] {
            assert!(validate_name(name).is_err(), "{name:?} must be refused");
        }
    }
}
