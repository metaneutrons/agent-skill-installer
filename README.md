# agent-skill-installer

[![crates.io](https://img.shields.io/crates/v/agent-skill-installer.svg)](https://crates.io/crates/agent-skill-installer)
[![docs.rs](https://img.shields.io/docsrs/agent-skill-installer)](https://docs.rs/agent-skill-installer)
[![CI](https://img.shields.io/github/actions/workflow/status/metaneutrons/agent-skill-installer/ci.yml?branch=main&label=CI)](https://github.com/metaneutrons/agent-skill-installer/actions/workflows/ci.yml)
[![License: LGPL-3.0-or-later](https://img.shields.io/badge/license-LGPL--3.0--or--later-blue.svg)](https://github.com/metaneutrons/agent-skill-installer/blob/main/LICENSE)
[![MSRV: 1.99](https://img.shields.io/badge/MSRV-1.99-orange.svg)](https://github.com/metaneutrons/agent-skill-installer/blob/main/Cargo.toml)

A Rust library that installs an [Agent Skill](https://agentskills.io/specification)
shipped inside your program into the skill directories of the AI coding tools
on the user's machine, so those tools know what your program is for and how to
use it. It finds the tools that are installed, plans the fewest copies that
reach all of them, installs only after the user agreed, and later keeps its own
unchanged copies current without asking. The crate decides what would happen
and carries it out; your program asks the user in its own CLI, TUI or GUI.

**Status: implemented, not yet released.** The design and its acceptance
criteria are in [`docs/plans/`](https://github.com/metaneutrons/agent-skill-installer/tree/main/docs/plans),
the evidence about the tools in
[`docs/tools.md`](https://github.com/metaneutrons/agent-skill-installer/blob/main/docs/tools.md).

## Features

- **Only where a tool is present.** A tool counts as installed when its home
  directory exists, and a skill directory is only created inside such a home.
  The crate never creates a tool's home.
- **The fewest copies that reach every tool.** The planner compares every
  combination of directories and picks the one that leaves the fewest tools
  seeing the skill twice, then needs the fewest copies, and names any tool that
  will see it twice anyway. Symlinks are resolved, so two names for one
  directory are one target.
- **Consent first.** The first installation into a directory happens only
  after the user agreed. A "no" is kept in a file your program chooses, and
  that directory is not asked about again.
- **Silent updates of its own work only.** The skill's frontmatter records
  which program installed it, the version and a digest over every file. A newer
  program updates an unchanged copy without asking, also inside a git working
  tree, which is reported with its root.
- **Never half-written.** A new version is written beside the skills directory
  and renamed into place; on a failure the old one is put back.
- **Extensible.** The built-in list covers six tools; a program can add its own
  tools and directories, and the planner treats them like the built-in ones.
- **Small.** Depends on `semver` and `sha2` only. No YAML parser, no network
  access, no terminal input or output.

## Installation

```sh
cargo add agent-skill-installer
```

or in `Cargo.toml`:

```toml
[dependencies]
agent-skill-installer = "0.1"
```

## Quick start

```rust,no_run
use agent_skill_installer::{
    Approval, Catalog, Consent, Environment, Installer, Program, Skill,
};

let skill = Skill::new(
    "devserial",
    "Serial terminal with a SQLite-buffered capture. Use it to read, search and write serial ports.",
    "# devserial\n\nRead the capture with `devserial read PORT --tail 50`.\n",
)?
.with_field("license", "GPL-3.0-or-later")?
.with_file("references/cli.md", "# Commands\n")?;
let program = Program::new("devserial", env!("CARGO_PKG_VERSION"))?;
let catalog = Catalog::builtin();
let env = Environment::from_process().expect("a home directory");
let installer = Installer::new(&catalog, &env, &skill, &program);

// At every start: keep earlier installations current. Asks nobody.
installer.update()?;

// When the user is asked: show what would happen, install what they agree to.
let mut consent = Consent::load("/path/in/your/data/dir/skill-consent")?;
let assessment = installer.assess(&consent)?;
let mut approval = Approval::default();
for step in assessment.awaiting_approval() {
    let agreed = true; // ask in your own surface, naming step.place and step.tools
    if agreed {
        approval.install.push(step.place.clone());
    } else {
        consent.decline(&step.place)?;
    }
}
consent.save()?;
for outcome in installer.apply(&assessment, &approval) {
    println!("{}: {}", outcome.place.display(), outcome.done);
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

`assess` writes nothing. It returns one step per place with what is there
(`Found`) and what would happen (`Action`): install, update, nothing and why,
or a problem to report. The `Plan` inside the assessment lists the installed
tools, the tools that will see the skill twice, and any tool that cannot be
served, with the reason. `remove` takes away every unchanged copy the program
installed.

To add a tool the built-in list leaves out:

```rust
use agent_skill_installer::{Catalog, Location, SkillDir, Tool};

let mut catalog = Catalog::builtin();
catalog
    .add_dir(SkillDir {
        id: "kiro".into(),
        path: Location::home(".kiro/skills"),
        requires: Some(Location::home(".kiro")),
        preferred: false,
    })?
    .add_tool(Tool {
        id: "kiro".into(),
        name: "Kiro".into(),
        home: Location::home(".kiro"),
        reads: vec!["kiro".into()],
        may_read: vec![],
    })?;
# Ok::<(), agent_skill_installer::CatalogError>(())
```

## Consent and updates

Consent is given for a place, not for a version. What `assess` decides for a
place depends on what it finds there under the skill's name:

| Found | Action |
| --- | --- |
| Nothing, the place is planned | Install once the user approves (`Action::Install`) |
| Nothing, the user declined the place | Nothing (`Why::Declined`) |
| This program's copy, unchanged, older | Update without asking (`Action::Update`) |
| This program's copy, unchanged, same or newer | Nothing; an older program never downgrades |
| This program's copy, changed by somebody | Reported (`Problem::Modified`); replaced only when listed in `Approval::overwrite` |
| A skill of that name that is not this program's | Reported (`Problem::Foreign`), never replaced |

A copy counts as this program's when its `metadata` block carries the record
the crate wrote, in the shape the crate writes it:

```yaml
metadata:
  installed-by: "devserial"
  installed-version: "0.3.0"
  content-digest: "sha256:…"
```

The digest covers every file of the skill directory with its own value blanked.
Any edited, added or removed file makes the copy "changed"; `.DS_Store` and
`Thumbs.db` do not. Copies of this program in places the plan no longer
chooses, for example for a tool uninstalled since, are still updated and
removed.

## Supported tools

The built-in catalog, from
[`docs/tools.md`](https://github.com/metaneutrons/agent-skill-installer/blob/main/docs/tools.md).
"Measured" means the behaviour was observed with the tool on a test machine;
"documented" means it follows the vendor's documentation only.

| Tool | Installed when this exists | Skill directories it reads | Evidence |
| --- | --- | --- | --- |
| Claude Code | `~/.claude` (`$CLAUDE_CONFIG_DIR` when set) | `~/.claude/skills` (`$CLAUDE_CONFIG_DIR/skills`) | measured |
| Codex | `~/.codex` (`$CODEX_HOME` when set) | `~/.agents/skills` | documented |
| Gemini CLI | `~/.gemini` | `~/.gemini/skills`, `~/.agents/skills` | measured |
| GitHub Copilot | `~/.copilot` (`$COPILOT_HOME` when set) | `~/.copilot/skills` (`$COPILOT_HOME/skills`), `~/.agents/skills`; in VS Code also `~/.claude/skills` | documented |
| Cursor | `~/.cursor` | `~/.cursor/skills`, `~/.agents/skills`, `~/.claude/skills` | documented |
| OpenCode | `~/.config/opencode` (`$XDG_CONFIG_HOME/opencode` when set) | `~/.config/opencode/skills` (`$XDG_CONFIG_HOME/opencode/skills`), `~/.claude/skills`, `~/.agents/skills` | documented; `XDG_CONFIG_HOME` from secondary sources |

Claude Code reads only `~/.claude/skills`, and Gemini CLI does not read it, so
a machine with both needs two copies. With Claude Code and Codex installed,
Cursor, OpenCode and Copilot in VS Code read both copies when they are
installed too; the plan names them in `Plan::duplicated`.

## Limits

- **Windows** paths follow the tools' documentation, which only ever says `~`.
  They are not verified, Windows is not part of CI, and the symlink cases are
  tested on Unix only.
- **Four of the six tools are documented, not measured**: Codex, Copilot,
  Cursor and OpenCode.
- **Two programs writing the same skill at the same moment** are not
  coordinated. Each write is atomic, so the place ends up with one complete
  version, but one of the two may report a failure.
- **An interrupted process** can leave a `.asi-*` staging directory beside the
  skills directory. It is not cleaned up automatically.
- **Package managers do not install skills.** That would be installing without
  consent; the program asks at runtime instead.

## Contributing

See [CONTRIBUTING.md](https://github.com/metaneutrons/agent-skill-installer/blob/main/CONTRIBUTING.md)
for the workflow and the checks CI runs, and
[SECURITY.md](https://github.com/metaneutrons/agent-skill-installer/blob/main/SECURITY.md)
for reporting a vulnerability.

## License

LGPL-3.0-or-later. See [LICENSE](https://github.com/metaneutrons/agent-skill-installer/blob/main/LICENSE).
