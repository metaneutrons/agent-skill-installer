# Initiative plan: agent-skill-installer

Epic: <https://github.com/metaneutrons/agent-skill-installer/issues/7>
Decision state: accepted on 11 October 2026; D3 decided the same day from M1

## Outcome and boundaries

A program that ships an [Agent Skill](https://agentskills.io/specification)
can put it where the AI coding tools on the user's machine read skills from, so
the tools know what the program is for and how to use it. The first two
consumers are devserial and devknx; both have a CLI, a TUI and a GUI, and both
already serve MCP with a one-line `instructions` string.

The crate decides *what* would happen and carries it out; the calling program
decides *whether*, by asking the user in its own surface. The crate never
prompts, prints or reads from a terminal.

Baseline, measured on the maintainer's machine on 11 October 2026:

- `~/.claude/skills` is a real directory with 20 skills, and it is a git
  working tree the maintainer uses as a backup of those skills.
- `~/.codex/skills` is a real directory whose entries are symlinks into
  `~/.claude/skills`.
- `~/.gemini/skills` is itself a symlink to `~/.claude/skills`.

So one physical directory is reachable under three names, and it is version
controlled. Both facts are design inputs, not edge cases.

Non-goals:

- **Project-level skills** (`<repo>/.claude/skills` and the like). Those belong
  to the repository's authors, not to a tool someone installed.
- **Installation by a package manager.** A Homebrew, APT or AUR post-install
  step writing into a home directory is exactly the installation without
  consent this crate exists to prevent.
- **Tools outside the mainstream.** They are not in the built-in list; a
  consuming program can add them.
- **Fetching skill content from the network.** The skill ships inside the
  program, so it always matches the version that runs.

## Design and decisions

### D1: Built-in targets

From the research of 11 October 2026 (primary sources linked in M1). All six
tools read the `<dir>/<name>/SKILL.md` layout.

| Tool | User-level skill directories it reads |
| --- | --- |
| Claude Code | `~/.claude/skills` |
| Codex CLI | `~/.agents/skills`; `~/.codex/skills` is no longer documented and survives only in Cursor's compatibility list |
| Gemini CLI | `~/.gemini/skills`, `~/.agents/skills` (the alias takes precedence) |
| GitHub Copilot | `~/.copilot/skills` (relocated by `COPILOT_HOME`), `~/.agents/skills`; VS Code also `~/.claude/skills` |
| Cursor | `~/.cursor/skills`, `~/.agents/skills`, `~/.claude/skills`, `~/.codex/skills` |
| OpenCode | `~/.config/opencode/skills`, `~/.claude/skills`, `~/.agents/skills` |

Two directories reach every tool in the table: `~/.claude/skills` for Claude
Code, which reads nothing else, and the vendor-neutral `~/.agents/skills` for
the other five. Tool-specific directories are the fallback for a tool whose
version predates `~/.agents/skills`.

The list is data, not code paths: `Target { tool, path, reads }` entries the
crate ships, and a consuming program can append its own.

### D2: When a target counts as present

A target is eligible only when the tool that reads it is evidently installed:
its own home directory exists (`~/.claude`, `~/.codex`, `~/.gemini`,
`~/.copilot`, `~/.cursor`, `~/.config/opencode`). Then creating the `skills/`
subdirectory inside that home is acceptable, because that is where the tool
looks. `~/.agents/skills` is eligible when at least one installed tool reads it.
The crate never creates a tool's home directory.

### D3: Fewest copies that reach every installed tool

Several tools read more than one directory, and OpenCode is reported to warn
about a skill name it finds twice. Installing everywhere is therefore wrong.
The planner picks the smallest set of physical directories that covers every
installed tool, after resolving symlinks, so `~/.gemini/skills` pointing at
`~/.claude/skills` counts as one place.

**Decided on 11 October 2026 from the M1 measurement**
([`docs/tools.md`](../tools.md)):

- **Copies, not symlinks.** Gemini CLI warns about a symlink to the same skill
  exactly as about a copy, so a symlink buys nothing. Each copy carries its own
  ownership record and is checked and updated on its own.
- **Selection rule.** Over the installed tools, the planner chooses the set of
  directories that, first, leaves the fewest tools seeing the skill more than
  once, then has the fewest directories, then prefers the vendor-neutral
  `~/.agents/skills` over a tool-specific one. With at most eight candidate
  directories the planner can try every set, which keeps the rule exact and
  testable.
- **What stays unavoidable.** Claude Code reads only `~/.claude/skills` and
  Codex only `~/.agents/skills`; with both installed, Cursor, OpenCode and
  Copilot in VS Code read both and see the skill twice. The plan names the
  affected tools to the user rather than pretending otherwise.

### D4: Ownership without a marker file

The crate needs to know whether a skill on disk is one it installed and
whether anybody changed it since. A separate marker file would be a second
artefact in somebody else's directory that tools would see and might load.

The record lives in the skill's own frontmatter, in the `metadata` map the
specification provides for client-defined properties (string keys, string
values):

```yaml
metadata:
  installed-by: devserial
  installed-version: "0.3.0"
  content-digest: "sha256:…"
```

`content-digest` covers every file of the skill directory in a canonical order,
with that one value blanked while hashing. A skill whose digest still matches
is unmodified and ours; any other state is somebody else's work. The record
survives a copy or a backup, and anyone opening the file can see where it came
from.

Alternative kept in reserve: the program carries the digests of every skill
version it ever shipped, and the frontmatter carries nothing. That needs no
cooperation from the tools but a list that grows with every release; it is the
fallback if M1 finds a tool that refuses the `metadata` key. Copilot's
documentation does not list `metadata`, and its handling of unknown keys is
undocumented.

### D5: Consent and updates

| Situation | What happens |
| --- | --- |
| Eligible target, no skill of this name | Install only after consent. The plan names target, tool and path. |
| The user declined for a target | Nothing, and no new question for that target. A newly detected target is asked about on its own. |
| Our skill, unmodified, older version | Updated without asking. Consent was given for the place, not for a version. |
| Our skill, unmodified, same or newer version | Nothing. An older program never downgrades a newer skill. |
| Our skill, modified | Not touched. Reported once, with the way to overwrite on purpose. |
| A skill of this name that is not ours | Not touched, reported as a conflict. |

Declined targets are stored by the calling program in its own data directory,
through a small store the crate provides. Nothing about consent is written
into the tools' directories.

### D6: Git working trees

Before writing, the planner walks up from the resolved target and reports when
it lies inside a git working tree (a `.git` directory or file), naming the
root. No git library and no git process: presence of `.git` is enough to warn,
and the crate never stages, commits or ignores anything.

**Decided on 11 October 2026:** an automatic update proceeds inside a git
working tree as anywhere else. The files are the crate's own and unchanged, so
updating them is what the user consented to; the report names the repository
so the change can be committed. Treating a working tree like a modified skill
was the alternative, and it would leave a stale skill in exactly the setup
where the user keeps skills deliberately.

### D7: Writing safely

- **Atomic replacement:** a new version is written to a temporary directory
  next to the target and moved into place, so a tool never reads a half-written
  skill.
- **Valid names only:** the skill name is validated against the specification
  before it becomes a path (already in `validate_name`).
- **No escape through symlinks:** a skill directory that is a symlink to
  somewhere outside the resolved target is never written through.
- **Removal is narrow:** only skill directories whose record and digest prove
  them ours and unmodified are removed.

### D8: Boundaries of the API

The crate takes the skill as name plus files in memory and the program's name
and version. It returns a plan of per-target actions with reasons, applies a
plan the caller approved, and runs the silent update. Surfaces (CLI, TUI, GUI)
stay in the consuming program; the crate has no terminal, prompt or logging
dependency.

## Delivery and acceptance

### M1: Tool behaviour measured, not assumed

Execution: <https://github.com/metaneutrons/agent-skill-installer/issues/2>
Dependencies: none

- M1-A1: A table in `docs/tools.md` records, per built-in tool, the
  directories it reads, the relocating environment variable and the support
  version, each with a primary-source URL.
- M1-A2: For every built-in tool available on a test machine, a skill with the
  D4 `metadata` block is shown to load. For a tool that cannot be run, the
  table says so instead of claiming a result.
- M1-A3: The duplicate behaviour from D3 is measured for every built-in tool
  that reads more than one directory: copy in both, symlink in one, copy in
  one. D3's open question is closed in the plan with that evidence.
- M1-A4: Whether `CLAUDE_CONFIG_DIR` and `CODEX_HOME` relocate the skill
  directory is measured.

### M2: The planner

Execution: <https://github.com/metaneutrons/agent-skill-installer/issues/3>
Dependencies: M1 for the final target list; work on the mechanics can start
before it.

- M2-A1: Detection follows D2, tested against a temporary home with every
  combination of present and absent tool homes.
- M2-A2: Targets resolve through symlinks and deduplicate; the maintainer's
  layout from the baseline, rebuilt in a temporary directory, yields one
  physical target.
- M2-A3: The plan selects the fewest copies per D3, with the rule M1 settled.
- M2-A4: A target inside a git working tree is reported with its root (D6).
  Counter-probe: the same layout without `.git` reports nothing.
- M2-A5: A consuming program can add a target, and the planner treats it like
  a built-in one.

### M3: Install, update, remove

Execution: <https://github.com/metaneutrons/agent-skill-installer/issues/4>
Dependencies: M2

- M3-A1: The ownership record follows D4; editing any byte of any file of an
  installed skill makes it "modified". Counter-probe: an untouched skill stays
  "unmodified".
- M3-A2: Every row of the D5 table has a test that sets up the situation and
  checks the action taken and the files left behind.
- M3-A3: Writes are atomic (D7). A test interrupts between write and move and
  shows the old skill intact.
- M3-A4: No write leaves the resolved target: a symlinked skill directory
  pointing elsewhere, a `..` name and an absolute path in a skill file name are
  each refused.
- M3-A5: The consent store records declined targets per D5, in a path the
  caller supplies.

### M4: First release

Execution: <https://github.com/metaneutrons/agent-skill-installer/issues/5>
Dependencies: M3

- M4-A1: 0.1.0 is published to crates.io by hand once, the trusted publisher
  is bound to `release.yml` and the `release` environment, and the following
  release goes through the pipeline without a stored token.
- M4-A2: The README documents the API with a working example that is compiled
  as a doctest.

### M5: devserial and devknx use it

Execution: <https://github.com/metaneutrons/agent-skill-installer/issues/6>, with narrower issues in both repositories
Dependencies: M4

- M5-A1: Each program builds its skill from its own sources (commands, routes,
  MCP tools) so the skill cannot drift from the binary, and ships it inside the
  binary.
- M5-A2: The CLI offers status, install and remove; TUI and GUI ask once when
  an undecided eligible target exists.
- M5-A3: GUI, TUI and daemon run the silent update at start. MCP mode runs it
  without writing to stdout.

Initiative completion: M1 to M5 accepted and both programs released with the
integration.

## Migration, risks and verification cost

- **Moving targets.** The tools change where they read skills from; Codex
  already deprecated `~/.codex/skills`. The built-in list will need updates,
  which is why it is data and why a program can extend it.
- **Writing into a user's version control.** On the maintainer's machine every
  install lands in a git working tree. D6 makes that visible; it does not make
  it harmless.
- **Measurement needs the tools.** M1 can only measure tools that run on an
  available machine. Unmeasured behaviour stays marked as such, and the owner
  accepted on 11 October 2026 that M1 can be accepted with tools that were not
  available marked as unmeasured.
- No data migration: nothing is installed today.

## Decision changes

- 11 October 2026: D6 decided, automatic updates proceed inside a git working
  tree. M3 no longer waits on it.
- 11 October 2026: M1 may be accepted with unavailable tools marked as
  unmeasured.
- 11 October 2026: D3 decided from the M1 measurement: copies rather than
  symlinks, and an exact selection rule. The Codex row of D1 now follows its
  documentation, which names only `~/.agents/skills`.
