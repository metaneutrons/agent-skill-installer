# Where the tools read skills, and what they do with them

The evidence behind D1 to D4 of [the plan](plans/agent-skill-installer.md),
collected for M1 on 11 October 2026. Every statement is either from the vendor's
own documentation, linked, or measured with
[`scripts/measure/tools.sh`](../scripts/measure/tools.sh). Anything neither
documented nor measured says so.

## Documented

| Tool | User-level skill directories | Relocated by | Support since | Source |
| --- | --- | --- | --- | --- |
| Claude Code | `~/.claude/skills` | `CLAUDE_CONFIG_DIR` (measured, see below) | announced 16 Oct 2025 | [docs](https://code.claude.com/docs/en/skills) |
| Codex CLI | `~/.agents/skills`; `/etc/codex/skills` for admins | not documented | not established | [docs](https://learn.chatgpt.com/docs/build-skills) |
| Gemini CLI | `~/.gemini/skills`, `~/.agents/skills` (alias wins) | not documented | 0.23.0 preview, default since 0.25.0 | [docs](https://geminicli.com/docs/cli/skills/), [changelog](https://geminicli.com/docs/changelogs/) |
| GitHub Copilot CLI | `~/.copilot/skills`, `~/.agents/skills` | `COPILOT_HOME` | announced 18 Dec 2025 | [docs](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-skills), [config dir](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference) |
| Copilot in VS Code | `~/.copilot/skills`, `~/.claude/skills`, `~/.agents/skills` | not documented | stable early January 2026 | [docs](https://code.visualstudio.com/docs/copilot/customization/agent-skills) |
| Cursor | `~/.cursor/skills`, `~/.agents/skills`, and for compatibility `~/.claude/skills`, `~/.codex/skills` | not documented | 2.4, 22 Jan 2026 | [docs](https://cursor.com/docs/context/skills), [changelog](https://cursor.com/changelog/2-4) |
| OpenCode | `~/.config/opencode/skills`, `~/.claude/skills`, `~/.agents/skills` | `XDG_CONFIG_HOME`, secondary sources only | not established | [docs](https://opencode.ai/docs/skills/) |

Notes from the same sources:

- **Codex** no longer documents `~/.codex/skills`; it survives only as a
  compatibility location in Cursor's list. The Codex documentation says skills
  with the same `name` are not merged and both can appear.
- **Names:** the strictest rules are VS Code's (an invalid name makes the skill
  "silently fail to load") and OpenCode's `^[a-z0-9]+(-[a-z0-9]+)*$`; both
  require the name to match the directory. `validate_name` enforces exactly
  that.
- **`metadata`** is a field of the [specification](https://agentskills.io/specification)
  (string keys, string values). Claude Code, Cursor and OpenCode list it;
  Copilot's documentation does not, and does not say how it treats keys it does
  not know.
- **OpenCode** asks to "ensure skill names are unique across all locations"
  and does not say what happens otherwise.
- **Windows:** no tool documents a Windows path; all write `~`, which is taken
  to be the user profile directory. Not verified.

## Measured

Machine: macOS 26.5.1, arm64. Claude Code 2.1.285, Gemini CLI 0.42.0. Every
scenario ran in a fresh temporary `HOME`; no model was called. Claude Code's
count comes from its debug log, Gemini CLI's list from `gemini skills list
--all`.

| Scenario | Claude Code | Gemini CLI |
| --- | --- | --- |
| Skill in the tool's own directory | loaded | loaded |
| Ownership record from D4 in `metadata` | loaded | loaded |
| An unknown top-level frontmatter key | loaded | loaded |
| Skill only in `~/.agents/skills` | **not loaded** | loaded |
| Skill only in `~/.claude/skills` | — | **not loaded** |
| Skill directory is a symlink to elsewhere | loaded | not measured |
| The whole skills directory is a symlink | loaded | loaded |
| `CLAUDE_CONFIG_DIR` points elsewhere | loads from `$CLAUDE_CONFIG_DIR/skills` | — |
| Same name copied into both directories it reads | — | loaded once from `~/.agents/skills`, **with a conflict warning** |
| Same name, the second a symlink to the first | — | loaded once, **with the same conflict warning** |
| Counter-probe: a directory without `SKILL.md` | not loaded | — |

What follows from it:

1. **Claude Code reads only `~/.claude/skills`**, and **Gemini CLI does not read
   it**. A machine with both needs at least two directories.
2. **A symlink does not avoid a duplicate.** Gemini compares locations, not
   resolved paths, and warns about a symlink to the same directory exactly as
   it does about a copy.
3. **The ownership record is safe in both measured tools**, and so is an
   unknown key; neither tool rejects or warns.

## Not measured

Codex CLI, Copilot CLI, Copilot in VS Code, Cursor and OpenCode are not
installed on the measuring machine. Their rows above are documentation only.
Per the decision of 11 October 2026, M1 is accepted with them marked as
unmeasured. Whether Cursor, OpenCode and Copilot in VS Code warn about or
duplicate a skill found in both `~/.claude/skills` and `~/.agents/skills` is
the most important open question among them.

To repeat the measurement on another machine:

```bash
scripts/measure/tools.sh
```
