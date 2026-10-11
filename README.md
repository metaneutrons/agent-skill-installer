# agent-skill-installer

Install an [Agent Skill](https://agentskills.io/specification) that ships with
your program into the skill directories of the AI coding tools on the user's
machine, so the tools know what your program is for and how to use it.

**Status: implemented, not yet released.** The design and its acceptance
criteria are in [`docs/plans/`](https://github.com/metaneutrons/agent-skill-installer/tree/main/docs/plans),
the evidence about the tools in
[`docs/tools.md`](https://github.com/metaneutrons/agent-skill-installer/blob/main/docs/tools.md).

## What it does

- **Only where a tool is present.** A tool counts as installed when its home
  directory exists, and a skill directory is only created inside such a home.
  The built-in list covers Claude Code, Codex, Gemini CLI, GitHub Copilot,
  Cursor and OpenCode; a program can add its own tools.
- **The fewest copies that reach every tool.** Claude Code reads only
  `~/.claude/skills`; most other tools read the vendor-neutral
  `~/.agents/skills`. The planner picks the combination that leaves the fewest
  tools seeing the skill twice, then the fewest copies, and names any tool that
  will see it twice anyway. Symlinks are resolved, so two names for one
  directory are one target.
- **Consent first.** The first installation into a directory happens only
  after the user agreed. The crate says what would happen and why; your
  program asks in its own CLI, TUI or GUI. A "no" is remembered in a file your
  program chooses.
- **Updates without asking, but only of its own work.** The skill's frontmatter
  records which program installed it, the version, and a digest over every
  file. A newer version of your program updates an unchanged copy silently,
  also inside a git working tree. A copy somebody changed is left alone and
  reported, and a skill of the same name that is not yours is never touched.
- **Never half-written.** A new version is written beside the skills directory
  and moved into place in one step; on any failure the old one is put back.

## Usage

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

`assess` returns one step per place with what is there (`Found`) and what would
happen (`Action`): install, update, nothing and why, or a problem to report.
`Plan` inside the assessment lists the installed tools, the tools that will see
the skill twice, and any tool that cannot be served, with the reason.

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

## Limits

- **Windows** paths follow the tools' documentation, which only ever says `~`;
  they are not verified there, and the symlink cases are tested on Unix only.
- **Four of the six tools are documented, not measured**: Codex, Copilot,
  Cursor and OpenCode. See `docs/tools.md`.
- **Two programs writing the same skill at the same moment** are not
  coordinated. Each write is atomic; the later one wins.
- **Package managers do not install skills.** That would be installing without
  consent; the program asks at runtime instead.

## License

LGPL-3.0-or-later. See [LICENSE](https://github.com/metaneutrons/agent-skill-installer/blob/main/LICENSE).
