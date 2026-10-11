// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! Where a skill should go on this machine, and why.
//!
//! The planner decides nothing a user has to agree to and writes nothing. It
//! reports which tools are installed, which directories they read, which of
//! those are one physical place, which copies serve every installed tool best,
//! and which tool would still see the skill twice.

use std::path::PathBuf;

use crate::catalog::Catalog;
use crate::env::Environment;
use crate::resolve;

/// The most physical places the planner compares every combination of. The
/// built-in catalog has six; a program adding more than this many is told so
/// rather than left waiting on an exponential search.
const MAX_PLACES: usize = 16;

/// One directory the skill would be written to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The physical directory, every symlink resolved. This is where the
    /// skill is written.
    pub path: PathBuf,
    /// The catalog directories that lead here, in catalog order. More than
    /// one when a symlink joins them.
    pub dirs: Vec<String>,
    /// The installed tools that read a skill here, in catalog order.
    pub tools: Vec<String>,
    /// Whether the directory has to be created.
    pub create: bool,
    /// The root of the git working tree it lies in, if any.
    pub git_root: Option<PathBuf>,
}

/// A tool the planner found installed but cannot serve, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unserved {
    /// The tool.
    pub tool: String,
    /// Why no directory it reads can be used.
    pub reason: String,
}

/// What the planner found and proposes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// Installed tools, in catalog order.
    pub installed: Vec<String>,
    /// Where the skill would be written.
    pub targets: Vec<Target>,
    /// Tools that would see the skill more than once with these targets. Empty
    /// unless that cannot be avoided.
    pub duplicated: Vec<String>,
    /// Installed tools no target reaches.
    pub unserved: Vec<Unserved>,
}

/// Why no plan could be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// More physical places than the planner compares exhaustively.
    TooManyPlaces(usize),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyPlaces(n) => write!(
                f,
                "{n} candidate places is more than the {MAX_PLACES} the planner compares"
            ),
        }
    }
}

impl std::error::Error for PlanError {}

/// A catalog directory as found on this machine.
struct Found {
    /// Index of the physical place it leads to, when it can be used.
    place: Option<usize>,
    /// Why it cannot be used, when it cannot.
    refused: Option<String>,
}

/// A physical place one or more usable directories lead to.
struct Place {
    path: PathBuf,
    dirs: Vec<usize>,
    preferred: bool,
    exists: bool,
}

/// Work out where a skill should go.
///
/// # Errors
/// Returns an error only for a catalog too large to plan exhaustively.
pub fn plan(catalog: &Catalog, env: &Environment) -> Result<Plan, PlanError> {
    let installed: Vec<usize> = catalog
        .tools
        .iter()
        .enumerate()
        .filter(|(_, tool)| tool.home.resolve(env).is_dir())
        .map(|(index, _)| index)
        .collect();

    // Only directories an installed tool reads are candidates; a directory
    // nobody installed reads is not ours to create.
    let wanted = |dir: usize| {
        let id = &catalog.dirs[dir].id;
        installed.iter().any(|&tool| {
            let tool = &catalog.tools[tool];
            tool.reads.contains(id) || tool.may_read.contains(id)
        })
    };

    let mut places: Vec<Place> = Vec::new();
    let found: Vec<Found> = (0..catalog.dirs.len())
        .map(|index| {
            if !wanted(index) {
                return Found {
                    place: None,
                    refused: Some("no installed tool reads it".to_string()),
                };
            }
            locate(catalog, env, index, &mut places)
        })
        .collect();

    if places.len() > MAX_PLACES {
        return Err(PlanError::TooManyPlaces(places.len()));
    }

    // A tool is served through the places its reliably read directories lead
    // to, and sees a copy in every place any directory it reads leads to,
    // once per directory: a tool does not resolve symlinks, as Gemini CLI's
    // conflict warning for a symlinked duplicate shows (docs/tools.md).
    let place_of = |id: &String| catalog.dir_index(id).and_then(|dir| found[dir].place);
    let serving: Vec<(usize, Vec<usize>, Vec<usize>)> = installed
        .iter()
        .map(|&tool| {
            let tool_entry = &catalog.tools[tool];
            let reaches = tool_entry.reads.iter().filter_map(place_of).collect();
            let sees = tool_entry
                .reads
                .iter()
                .chain(&tool_entry.may_read)
                .filter_map(place_of)
                .collect();
            (tool, reaches, sees)
        })
        .collect();

    let unserved = unserved(catalog, &found, &serving);

    let servable: Vec<&(usize, Vec<usize>, Vec<usize>)> = serving
        .iter()
        .filter(|(_, reaches, _)| !reaches.is_empty())
        .collect();
    let chosen = choose(&places, &servable);

    let targets = chosen
        .iter()
        .map(|&place| {
            let entry = &places[place];
            Target {
                path: entry.path.clone(),
                dirs: entry
                    .dirs
                    .iter()
                    .map(|&dir| catalog.dirs[dir].id.clone())
                    .collect(),
                tools: servable
                    .iter()
                    .filter(|(_, reaches, _)| reaches.contains(&place))
                    .map(|(tool, _, _)| catalog.tools[*tool].id.clone())
                    .collect(),
                create: !entry.exists,
                git_root: resolve::git_root(&entry.path),
            }
        })
        .collect();

    let duplicated = servable
        .iter()
        .filter(|(_, _, sees)| sees.iter().filter(|place| chosen.contains(place)).count() > 1)
        .map(|(tool, _, _)| catalog.tools[*tool].id.clone())
        .collect();

    Ok(Plan {
        installed: installed
            .iter()
            .map(|&tool| catalog.tools[tool].id.clone())
            .collect(),
        targets,
        duplicated,
        unserved,
    })
}

/// Installed tools no usable directory reaches, with the reason per directory.
fn unserved(
    catalog: &Catalog,
    found: &[Found],
    serving: &[(usize, Vec<usize>, Vec<usize>)],
) -> Vec<Unserved> {
    serving
        .iter()
        .filter(|(_, reaches, _)| reaches.is_empty())
        .map(|(tool, _, _)| {
            let tool = &catalog.tools[*tool];
            let reasons: Vec<String> = tool
                .reads
                .iter()
                .filter_map(|id| {
                    let dir = catalog.dir_index(id)?;
                    found[dir]
                        .refused
                        .as_ref()
                        .map(|why| format!("{id}: {why}"))
                })
                .collect();
            Unserved {
                tool: tool.id.clone(),
                reason: reasons.join("; "),
            }
        })
        .collect()
}

/// Resolve one catalog directory and file it under its physical place.
fn locate(catalog: &Catalog, env: &Environment, index: usize, places: &mut Vec<Place>) -> Found {
    let dir = &catalog.dirs[index];
    let path = dir.path.resolve(env);
    let exists = path.is_dir();
    if !exists
        && let Some(requires) = &dir.requires
        && !requires.resolve(env).is_dir()
    {
        // Creating the directory would create a tool's home too, which is
        // never ours to do (D2).
        return Found {
            place: None,
            refused: Some(format!(
                "{} does not exist",
                requires.resolve(env).display()
            )),
        };
    }
    match resolve::physical(&path) {
        Ok(physical) => {
            let place = places.iter().position(|place| place.path == physical);
            let place = place.unwrap_or_else(|| {
                places.push(Place {
                    path: physical,
                    dirs: Vec::new(),
                    preferred: false,
                    exists,
                });
                places.len() - 1
            });
            places[place].dirs.push(index);
            places[place].preferred |= dir.preferred;
            places[place].exists |= exists;
            Found {
                place: Some(place),
                refused: None,
            }
        }
        Err(error) => Found {
            place: None,
            refused: Some(format!("{} cannot be resolved: {error}", path.display())),
        },
    }
}

/// The set of places D3 asks for.
///
/// Every combination is compared. Among those that reach every servable tool,
/// the winner leaves the fewest tools seeing the skill twice, then has the
/// fewest places, then the most preferred ones, then comes first in catalog
/// order, so the same machine always gets the same plan.
fn choose(places: &[Place], servable: &[&(usize, Vec<usize>, Vec<usize>)]) -> Vec<usize> {
    if servable.is_empty() {
        return Vec::new();
    }
    let mut best: Option<((usize, usize, usize), Vec<usize>)> = None;
    for mask in 1u32..(1u32 << places.len()) {
        let set: Vec<usize> = (0..places.len()).filter(|i| mask & (1 << i) != 0).collect();
        if !servable
            .iter()
            .all(|(_, reaches, _)| reaches.iter().any(|place| set.contains(place)))
        {
            continue;
        }
        let duplicates = servable
            .iter()
            .filter(|(_, _, sees)| sees.iter().filter(|place| set.contains(place)).count() > 1)
            .count();
        let unpreferred = set
            .iter()
            .filter(|&&place| !places[place].preferred)
            .count();
        let score = (duplicates, set.len(), unpreferred);
        // Masks count up, so the first set with a given score is the one that
        // comes first in catalog order; only a strictly better score replaces
        // it.
        if best.as_ref().is_none_or(|(known, _)| score < *known) {
            best = Some((score, set));
        }
    }
    best.map(|(_, set)| set).unwrap_or_default()
}

/// Every existing physical directory of the catalog, resolved and without
/// duplicates.
///
/// Updates and removals look here rather than only at a plan's targets: a
/// skill installed for a tool that has since been uninstalled is still the
/// program's own, and still worth keeping current or taking away.
#[must_use]
pub fn existing_places(catalog: &Catalog, env: &Environment) -> Vec<PathBuf> {
    let mut places: Vec<PathBuf> = Vec::new();
    for dir in &catalog.dirs {
        let path = dir.path.resolve(env);
        if !path.is_dir() {
            continue;
        }
        if let Ok(physical) = resolve::physical(&path)
            && !places.contains(&physical)
        {
            places.push(physical);
        }
    }
    places
}
