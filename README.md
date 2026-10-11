# agent-skill-installer

Install an [Agent Skill](https://agentskills.io/specification) that ships with
your program into the skill directories of the AI coding tools on the user's
machine, so the tools know what your program is for and how to use it.

**Status: implemented, not yet released.** The design and its acceptance
criteria are in [`docs/plans/`](docs/plans/), the evidence about the tools in
[`docs/tools.md`](docs/tools.md).

The intended behaviour, in short:

- **Only where a tool is present.** The crate writes only into skill
  directories that already exist, and it covers the mainstream tools only. A
  program can add its own targets to the list.
- **Consent first.** The first installation into a directory happens only
  after the user agreed. The crate provides the plan, and the calling program
  asks in its own CLI, TUI or GUI.
- **Updates without asking, but only of its own work.** When a newer version
  of the program runs, a skill it installed earlier is updated, provided
  nobody changed it since. A changed skill is left alone and reported.
- **Aware of the surroundings.** A target that resolves to the same place as
  another, through a symlink, is written once. A target inside a git working
  tree is reported as such.

## License

LGPL-3.0-or-later. See [LICENSE](LICENSE).
