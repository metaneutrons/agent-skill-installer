#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-3.0-or-later
# Copyright (C) 2026 Fabian Schmieder
#
# M1 of docs/plans/agent-skill-installer.md: measure, per installed tool,
# which user-level directories it loads skills from, whether it accepts the
# ownership record in the frontmatter, and what it does with the same skill in
# two places.
#
# Every scenario runs in a fresh temporary HOME, so the user's real skill
# directories are never read or written. No model is called: Claude Code logs
# what it loaded before it checks the login, and Gemini CLI lists skills
# without one.
#
# Usage: scripts/measure/tools.sh            # every tool that is installed
# Output: one line per scenario, "tool scenario result".

set -euo pipefail

probe() { # $1 dir, $2 name, $3 extra frontmatter lines
  mkdir -p "$1/$2"
  {
    printf -- '---\nname: %s\ndescription: Probe skill for agent-skill-installer M1. Does nothing.\n' "$2"
    printf '%b' "${3:-}"
    printf -- '---\nProbe.\n'
  } > "$1/$2/SKILL.md"
}

ownership_record='metadata:\n  installed-by: asi-measure\n  installed-version: "0.1.0"\n  content-digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000"\n'
unknown_key='asi-unknown-key: anything\n'

fresh_home() { mktemp -d "${TMPDIR:-/tmp}/asi-m1.XXXXXX"; }

# --------------------------------------------------------------- Claude Code

# Number of user skills Claude Code loaded, read from its debug log.
claude_user_skills() { # $1 HOME, [$2 CLAUDE_CONFIG_DIR]
  local home=$1 cfg=${2:-$1/.claude}
  # Run from a directory of its own: from HOME itself, `.claude/skills` would
  # also count as the project's skills and the user count would mean less.
  mkdir -p "$home/work"
  (cd "$home/work" && env HOME="$home" CLAUDE_CONFIG_DIR="$cfg" \
     timeout 60 claude -p --debug x >/dev/null 2>&1 || true)
  local log
  log=$(ls -t "$cfg"/debug/*.txt 2>/dev/null | head -1)
  [ -n "$log" ] || { echo "no-debug-log"; return; }
  grep -oE 'Loaded [0-9]+ unique skills \([^)]*user: [0-9]+' "$log" | tail -1 |
    sed -E 's/.*user: ([0-9]+)/\1/'
}

measure_claude() {
  command -v claude >/dev/null || { echo "claude not-installed"; return; }
  local h
  h=$(fresh_home); probe "$h/.claude/skills" asi-probe-plain
  echo "claude plain-in-claude-skills user_skills=$(claude_user_skills "$h")"

  h=$(fresh_home); probe "$h/.claude/skills" asi-probe-record "$ownership_record"
  echo "claude ownership-record user_skills=$(claude_user_skills "$h")"

  h=$(fresh_home); probe "$h/.claude/skills" asi-probe-unknown "$unknown_key"
  echo "claude unknown-top-level-key user_skills=$(claude_user_skills "$h")"

  h=$(fresh_home); mkdir -p "$h/.claude/skills/asi-probe-empty"
  echo "claude counter-probe-no-skill-md user_skills=$(claude_user_skills "$h")"

  h=$(fresh_home); mkdir -p "$h/.claude"; probe "$h/.agents/skills" asi-probe-agents
  echo "claude only-in-agents-skills user_skills=$(claude_user_skills "$h")"

  h=$(fresh_home); probe "$h/elsewhere" asi-probe-linked
  mkdir -p "$h/.claude/skills"; ln -s "$h/elsewhere/asi-probe-linked" "$h/.claude/skills/asi-probe-linked"
  echo "claude symlinked-skill-dir user_skills=$(claude_user_skills "$h")"

  h=$(fresh_home); probe "$h/elsewhere" asi-probe-linked-root
  mkdir -p "$h/.claude"; ln -s "$h/elsewhere" "$h/.claude/skills"
  echo "claude symlinked-skills-root user_skills=$(claude_user_skills "$h")"

  h=$(fresh_home); probe "$h/relocated/skills" asi-probe-relocated
  echo "claude claude-config-dir-relocates user_skills=$(claude_user_skills "$h" "$h/relocated")"
}

# ---------------------------------------------------------------- Gemini CLI

# Names and locations Gemini CLI lists, built-ins excluded.
gemini_skills() { # $1 HOME
  mkdir -p "$1/work"
  (cd "$1/work" && HOME="$1" timeout 60 gemini skills list --all 2>&1 || true) |
    awk '/Skill conflict detected/ {print "CONFLICT-WARNING"}
         /^asi-probe/ {name=$1}
         /Location:/ && name {sub(/.*Location: +/, ""); sub(/.*asi-m1\.[A-Za-z0-9]+\//, "~/"); print name "@" $0; name=""}' |
    sort | paste -sd ' ' -
}

measure_gemini() {
  command -v gemini >/dev/null || { echo "gemini not-installed"; return; }
  local h
  h=$(fresh_home); probe "$h/.gemini/skills" asi-probe-plain
  echo "gemini plain-in-gemini-skills [$(gemini_skills "$h")]"

  h=$(fresh_home); mkdir -p "$h/.gemini"; probe "$h/.agents/skills" asi-probe-agents
  echo "gemini only-in-agents-skills [$(gemini_skills "$h")]"

  h=$(fresh_home); mkdir -p "$h/.gemini"; probe "$h/.claude/skills" asi-probe-claude
  echo "gemini only-in-claude-skills [$(gemini_skills "$h")]"

  h=$(fresh_home); probe "$h/.gemini/skills" asi-probe-record "$ownership_record"
  echo "gemini ownership-record [$(gemini_skills "$h")]"

  h=$(fresh_home); probe "$h/.gemini/skills" asi-probe-unknown "$unknown_key"
  echo "gemini unknown-top-level-key [$(gemini_skills "$h")]"

  h=$(fresh_home); probe "$h/.gemini/skills" asi-probe-dup; probe "$h/.agents/skills" asi-probe-dup
  echo "gemini same-name-copied-twice [$(gemini_skills "$h")]"

  h=$(fresh_home); probe "$h/.agents/skills" asi-probe-dup
  mkdir -p "$h/.gemini/skills"; ln -s "$h/.agents/skills/asi-probe-dup" "$h/.gemini/skills/asi-probe-dup"
  echo "gemini same-name-symlinked-twice [$(gemini_skills "$h")]"

  h=$(fresh_home); probe "$h/.claude/skills" asi-probe-root
  mkdir -p "$h/.gemini"; ln -s "$h/.claude/skills" "$h/.gemini/skills"
  echo "gemini skills-root-symlinked [$(gemini_skills "$h")]"
}

measure_claude
measure_gemini
