use clap::{Parser, Subcommand, ValueEnum};
use spec_autonomous_core::{Framework, detect};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "spec-autonomous",
    version,
    about = "Lightweight spec orchestration CLI — bootstrap with read-only detection"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Detect OpenSpec and Spec Kit markers; does not execute repository scripts.
    Detect {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long, value_enum, default_value = "auto")]
        framework: Selection,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Selection {
    Auto,
    Openspec,
    Speckit,
}

fn main() {
    match Cli::parse().command {
        Command::Detect {
            path,
            framework,
            json,
        } => {
            let requested = match framework {
                Selection::Auto => None,
                Selection::Openspec => Some(Framework::Openspec),
                Selection::Speckit => Some(Framework::Speckit),
            };
            match detect(&path, requested) {
                Ok(report) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&report).unwrap());
                    } else {
                        println!("Repository: {}", report.root.display());
                        for item in &report.detected {
                            println!("{:?}: {}", item.framework, item.evidence.join(", "));
                        }
                        if report.detected.is_empty() {
                            println!("No supported spec framework detected.");
                        }
                        if report.ambiguous {
                            println!(
                                "Multiple frameworks detected; select with --framework openspec|speckit."
                            );
                        }
                        for warning in &report.warnings {
                            eprintln!("Warning: {warning}");
                        }
                    }
                }
                Err(error) => {
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({"schema_version": 1, "error": {"code": "detection_failed", "message": format!("{error:#}")}})
                        );
                    } else {
                        eprintln!("Error: {error:#}");
                    }
                    std::process::exit(2);
                }
            }
        }
    }
}
