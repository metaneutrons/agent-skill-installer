// SPDX-License-Identifier: LGPL-3.0-or-later
// Copyright (C) 2026 Fabian Schmieder

//! Try the installer on a real machine before using it in a program.
//!
//! ```text
//! cargo run --example try-it -- plan                  # what is installed, where it would go
//! cargo run --example try-it -- assess                # place by place, what would happen
//! cargo run --example try-it -- install --yes         # install where nothing is in the way
//! cargo run --example try-it -- update --version 0.2.0
//! cargo run --example try-it -- remove
//! ```
//!
//! `--home DIR` plans for DIR instead of the real home, which is the way to
//! try `install` without touching anything that matters. Without `--yes`,
//! `install` only shows what it would do. The skill is a harmless demo named
//! `asi-try-it`.

use std::path::PathBuf;

use agent_skill_installer::{AgentSkill, Environment, Program, Skill, Step};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "plan".to_string());
    let mut home = None;
    let mut version = "0.1.0".to_string();
    let mut yes = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--home" => home = args.next().map(PathBuf::from),
            "--version" => version = args.next().ok_or("--version needs a value")?,
            "--yes" => yes = true,
            other => return Err(format!("unknown argument {other}").into()),
        }
    }

    let env = match home {
        Some(home) => Environment::new(home),
        None => Environment::from_process().ok_or("no home directory")?,
    };
    let skill = Skill::new(
        "asi-try-it",
        "Demo skill installed by agent-skill-installer's try-it example. It does nothing; remove it with `cargo run --example try-it -- remove`.",
        "# asi-try-it\n\nA demo. Nothing to do here.\n",
    )?;
    let program = Program::new("asi-try-it", &version)?;
    let consent_file = env.home().join(".cache/asi-try-it/consent");
    let shown_home = env.home().to_path_buf();
    let agent = AgentSkill::new(skill, program, consent_file)?.with_environment(env);

    match command.as_str() {
        "plan" => {
            let plan = agent.status()?.plan;
            println!("home:       {}", shown_home.display());
            println!("installed:  {}", list(&plan.installed));
            for target in &plan.targets {
                println!(
                    "target:     {}{}\n            dirs {}, read by {}{}",
                    target.path.display(),
                    if target.create {
                        " (would be created)"
                    } else {
                        ""
                    },
                    list(&target.dirs),
                    list(&target.tools),
                    target
                        .git_root
                        .as_ref()
                        .map(|root| format!(
                            "\n            inside the git working tree {}",
                            root.display()
                        ))
                        .unwrap_or_default()
                );
            }
            println!("duplicated: {}", list(&plan.duplicated));
            for unserved in &plan.unserved {
                println!("unserved:   {} ({})", unserved.tool, unserved.reason);
            }
        }
        "assess" => {
            for step in &agent.status()?.steps {
                print_step(step);
            }
        }
        "install" => {
            let Some(offer) = agent.offer()? else {
                println!("nothing to offer: every place is decided");
                return Ok(());
            };
            if !yes {
                println!("would install into (add --yes to do it):");
                for step in offer.steps() {
                    println!("  {}", step.place.display());
                }
                return Ok(());
            }
            for outcome in offer.accept()? {
                println!("{}: {}", outcome.place.display(), outcome.done);
            }
        }
        "update" => {
            for outcome in agent.update()? {
                println!("{}: {}", outcome.place.display(), outcome.done);
            }
        }
        "remove" => {
            for outcome in agent.remove()? {
                println!("{}: {}", outcome.place.display(), outcome.done);
            }
        }
        other => return Err(format!("unknown command {other}").into()),
    }
    Ok(())
}

fn list(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}

fn print_step(step: &Step) {
    println!("{}", step.place.display());
    println!("  found:  {:?}", step.found);
    println!("  action: {:?}", step.action);
    if !step.tools.is_empty() {
        println!("  for:    {}", step.tools.join(", "));
    }
    if let Some(root) = &step.git_root {
        println!("  git:    {}", root.display());
    }
}
