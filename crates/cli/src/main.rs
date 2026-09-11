use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};
use spec_autonomous_core::{
    Framework,
    config::Config,
    engine,
    git::Repository,
    model::{Milestone, Plan, Range, Run},
    progress, provider, runner, skills,
    state::Store,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Parser)]
#[command(
    name = "spec-autonomous",
    version,
    about = "Autonomous orchestration over your native OpenSpec or Spec Kit workflow"
)]
struct Cli {
    /// Project directory; progress/resume resolve its shared Git common directory.
    #[arg(long, global = true, default_value = ".")]
    path: PathBuf,
    #[arg(long, global = true, value_enum)]
    framework: Option<Selection>,
    /// JSON output; run/resume emit NDJSON events followed by a final result.
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true, value_enum)]
    format: Option<Format>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Clone, Copy, ValueEnum)]
enum Selection {
    Auto,
    Openspec,
    Speckit,
}
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Human,
    Json,
    Toml,
}
#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    Native,
    Autonomous,
}
impl Mode {
    fn name(self) -> String {
        match self {
            Self::Native => "native",
            Self::Autonomous => "autonomous",
        }
        .into()
    }
}
#[derive(Args, Default)]
struct SourceArgs {
    #[arg(long,conflicts_with_all=["change","feature"])]
    milestone: Option<String>,
    #[arg(long, conflicts_with = "feature")]
    change: Option<String>,
    #[arg(long)]
    feature: Option<String>,
}
#[derive(Subcommand)]
enum Command {
    /// Read-only provider discovery, never executing repository scripts.
    Detect,
    /// Bind /autonomous and /auto (or native skill equivalents) to this repository.
    Init {
        #[arg(long)]
        agent: Option<String>,
        #[arg(long, default_value = "")]
        prefix: String,
    },
    /// Install or remove owned host skills without replacing native SDD commands.
    Skills {
        #[command(subcommand)]
        command: SkillCommand,
    },
    /// Check the selected framework, runner capabilities and configuration.
    Doctor {
        #[arg(long)]
        runner: Option<String>,
    },
    /// Create a roadmap from a goal using your existing SDD framework.
    Milestone {
        #[command(subcommand)]
        command: MilestoneCommand,
    },
    /// Read the versioned milestone roadmap.
    Roadmap {
        #[arg(long)]
        milestone: String,
    },
    /// Read native artifacts, task provenance, capabilities and next action.
    Inspect {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long)]
        phase: Option<String>,
    },
    /// Complete native planning and prepare the next phase execution DAG.
    Plan {
        #[command(flatten)]
        source: SourceArgs,
    },
    /// Execute a milestone or inclusive phase range with verification and repair.
    #[command(visible_aliases = ["autonomous", "auto"])]
    Run {
        #[command(flatten)]
        source: SourceArgs,
        #[arg(long,conflicts_with_all=["milestone","change","feature"])]
        plan: Option<PathBuf>,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long,conflicts_with_all=["from","to"])]
        only: Option<String>,
        #[arg(long, value_enum)]
        mode: Option<Mode>,
        #[arg(long)]
        autonomous: bool,
        #[arg(long)]
        max_workers: Option<usize>,
        #[arg(long)]
        delivery: Option<String>,
    },
    /// Read repository-wide progress across all Git worktrees.
    Progress {
        #[arg(long)]
        all_worktrees: bool,
    },
    /// Read one run, or the most recently updated run.
    Status { run_id: Option<String> },
    /// Reconcile and continue a run with its saved scope and policy.
    Resume {
        run_id: String,
        #[arg(long, value_enum)]
        mode: Option<Mode>,
        #[arg(long)]
        reload_config: bool,
        #[arg(long, default_value_t = 0)]
        extend_seconds: u64,
        #[arg(long)]
        max_attempts: Option<u32>,
    },
    /// Request a durable pause and stop active workers.
    Pause { run_id: String },
    /// Request cancellation while preserving worktrees and evidence.
    Cancel { run_id: String },
    /// Read the run report, evidence and event history.
    Report { run_id: String },
    /// Remove clean completed workers, retaining branches and evidence.
    Cleanup { run_id: String },
    /// Record an explicit outcome for an interrupted native hook.
    ResolveHook {
        run_id: String,
        #[arg(long)]
        key: String,
        #[arg(long)]
        outcome: String,
        #[arg(long)]
        evidence: String,
    },
}
#[derive(Subcommand)]
enum SkillCommand {
    Install {
        #[arg(long)]
        agent: String,
        #[arg(long, default_value = "project")]
        scope: String,
        #[arg(long, default_value = "")]
        prefix: String,
    },
    Uninstall,
}
#[derive(Subcommand)]
enum MilestoneCommand {
    New {
        goal: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, value_enum, default_value = "native")]
        mode: Mode,
        #[arg(long)]
        max_workers: Option<usize>,
    },
}
fn requested(value: Option<Selection>) -> Option<Framework> {
    match value {
        Some(Selection::Openspec) => Some(Framework::Openspec),
        Some(Selection::Speckit) => Some(Framework::Speckit),
        _ => None,
    }
}
fn source(root: &Path, args: &SourceArgs, framework: Option<Framework>) -> Result<Milestone> {
    if let Some(id) = &args.milestone {
        return provider::load_milestone(root, id);
    }
    let framework = provider::framework(root, framework)?;
    let selector = match framework {
        Framework::Openspec => args
            .change
            .clone()
            .context("selection_required: specify --change or --milestone")?,
        Framework::Speckit => provider::select_feature(root, args.feature.as_deref())?,
    };
    Ok(engine::source_milestone(framework, &selector))
}
fn emit(mut value: Value, format: Format) -> Result<()> {
    progress::portable(&mut value);
    match format {
        Format::Json => println!("{}", serde_json::to_string_pretty(&value)?),
        Format::Toml => println!("{}", toml::to_string_pretty(&value)?),
        Format::Human => println!("{}", progress::human(&value)),
    }
    Ok(())
}
fn emit_run(run: &Run, format: Format) -> Result<()> {
    let mut value = json!({"schema_version":1,"data":progress::public_run(run)});
    progress::portable(&mut value);
    if format == Format::Json {
        println!("{}", value);
        Ok(())
    } else {
        emit(value, format)
    }
}
fn observe(format: Format) -> impl FnMut(&Run, &str) {
    move |run, event| {
        let data = json!({"schema_version":1,"event":event,"data":progress::summary(run)});
        if format == Format::Json {
            println!("{}", data);
        } else {
            eprintln!("{} | {} | {}", run.id, run.stage, event);
        }
    }
}
fn run_exit(run: &Run) -> i32 {
    match run.status.as_str() {
        "completed" | "scope_completed" | "plan_ready" | "handed_off" => 0,
        "cancelled" => 130,
        _ => 4,
    }
}
fn execute(cli: Cli, format: Format) -> Result<i32> {
    let root = cli
        .path
        .canonicalize()
        .context("invalid_path: cannot resolve --path")?;
    if !root.is_dir() {
        bail!("invalid_path: --path must be a directory");
    }
    let framework = requested(cli.framework);
    if matches!(cli.command, Command::Detect) {
        let report = spec_autonomous_core::detect(&root, framework)?;
        emit(serde_json::to_value(report)?, format)?;
        return Ok(0);
    }
    let project = spec_autonomous_core::detect(&root, None)?.root;
    let mut config = Config::load(&project)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed))?;
    let mut observer = observe(format);
    let result = match cli.command {
        Command::Detect => unreachable!(),
        Command::Init { agent, prefix } => skills::init(&project, agent.as_deref(), &prefix)?,
        Command::Skills { command } => match command {
            SkillCommand::Install {
                agent,
                scope,
                prefix,
            } => {
                if scope != "project" {
                    bail!("scope_unsupported: use project scope");
                }
                json!({"installed":skills::install(&project,&agent,&prefix)?})
            }
            SkillCommand::Uninstall => json!({"removed":skills::uninstall(&project)?}),
        },
        Command::Doctor { runner: profile } => {
            if let Some(profile) = profile {
                config.runner.profile = profile;
            }
            json!({"framework":provider::framework(&project,framework)?,"runner":runner::doctor(&config)?})
        }
        Command::Roadmap { milestone } => {
            serde_json::to_value(provider::load_milestone(&project, &milestone)?)?
        }
        Command::Inspect {
            source: args,
            phase,
        } => {
            let m = source(&project, &args, framework)?;
            let p = if let Some(id) = phase {
                m.phases
                    .iter()
                    .find(|p| p.id == id || p.label == id)
                    .context("selection_required: phase not found")?
            } else if m.phases.len() == 1 {
                &m.phases[0]
            } else {
                bail!("selection_required: use --phase for a multi-phase milestone");
            };
            serde_json::to_value(provider::inspect(
                &project,
                m.framework,
                &p.source.selector,
                &config,
            )?)?
        }
        Command::Progress { all_worktrees: _ } => progress::snapshot(&root)?,
        Command::Status { run_id } => {
            let repo = Repository::discover(&root)?;
            let store = Store::open(&repo, false)?.context("run_not_found: no runtime state")?;
            let run = if let Some(id) = run_id {
                store.get(&id)?
            } else {
                store.list()?.into_iter().next().context("run_not_found")?
            };
            progress::public_run(&run)
        }
        Command::Report { run_id } => {
            let repo = Repository::discover(&root)?;
            let store = Store::open(&repo, false)?.context("run_not_found")?;
            json!({"run":progress::public_run(&store.get(&run_id)?),"events":store.events(&run_id)?,"report_path":spec_autonomous_core::state::report_path(&store.root,&run_id)})
        }
        Command::Cleanup { run_id } => spec_autonomous_core::cleanup::cleanup(&root, &run_id)?,
        Command::ResolveHook {
            run_id,
            key,
            outcome,
            evidence,
        } => progress::public_run(&engine::resolve_hook(
            &root, &run_id, &key, &outcome, &evidence,
        )?),
        Command::Pause { run_id } => control(&root, &run_id, "pause")?,
        Command::Cancel { run_id } => control(&root, &run_id, "cancel")?,
        Command::Run {
            source: args,
            plan: plan_path,
            from,
            to,
            only,
            mode,
            autonomous,
            max_workers,
            delivery,
        } => {
            if autonomous && matches!(mode, Some(Mode::Native)) {
                bail!("invalid_mode: --autonomous conflicts with --mode native");
            }
            if let Some(n) = max_workers {
                config.execution.max_workers = n;
            }
            if let Some(v) = delivery {
                config.execution.delivery = v;
            }
            let supplied_plan: Option<Plan> = plan_path
                .map(|p| -> Result<Plan> { Ok(toml::from_str(&fs::read_to_string(p)?)?) })
                .transpose()?;
            let m = if let Some(p) = &supplied_plan {
                p.milestone
                    .clone()
                    .context("invalid_plan: missing native milestone binding")?
            } else {
                source(&project, &args, framework)?
            };
            let run = engine::start(
                &project,
                engine::Start {
                    milestone: m,
                    range: Range { from, to, only },
                    mode: mode.map(Mode::name).unwrap_or_else(|| {
                        if autonomous {
                            "autonomous".into()
                        } else {
                            config.execution.mode.clone()
                        }
                    }),
                    create_roadmap: false,
                    plan: supplied_plan,
                },
                config,
                cancel,
                &mut observer,
            )?;
            let code = run_exit(&run);
            emit_run(&run, format)?;
            return Ok(code);
        }
        Command::Plan { source: args } => {
            let m = source(&project, &args, framework)?;
            let run = engine::start(
                &project,
                engine::Start {
                    milestone: m,
                    range: Range::default(),
                    mode: "plan".into(),
                    create_roadmap: false,
                    plan: None,
                },
                config,
                cancel,
                &mut observer,
            )?;
            let code = run_exit(&run);
            emit_run(&run, format)?;
            return Ok(code);
        }
        Command::Milestone {
            command:
                MilestoneCommand::New {
                    goal,
                    id,
                    mode,
                    max_workers,
                },
        } => {
            if let Some(n) = max_workers {
                config.execution.max_workers = n;
            }
            let m = engine::goal_milestone(id, goal, provider::framework(&project, framework)?);
            let run = engine::start(
                &project,
                engine::Start {
                    milestone: m,
                    range: Range::default(),
                    mode: mode.name(),
                    create_roadmap: true,
                    plan: None,
                },
                config,
                cancel,
                &mut observer,
            )?;
            let code = run_exit(&run);
            emit_run(&run, format)?;
            return Ok(code);
        }
        Command::Resume {
            run_id,
            mode,
            reload_config,
            extend_seconds,
            max_attempts,
        } => {
            let run = engine::resume_with(
                &root,
                &run_id,
                engine::ResumeOptions {
                    mode: mode.map(Mode::name),
                    reload_config,
                    extend_seconds,
                    max_attempts,
                },
                cancel,
                &mut observer,
            )?;
            let code = run_exit(&run);
            emit_run(&run, format)?;
            return Ok(code);
        }
    };
    emit(json!({"schema_version":1,"data":result}), format)?;
    Ok(0)
}
fn control(root: &Path, id: &str, action: &str) -> Result<Value> {
    let repo = Repository::discover(root)?;
    let store = Store::for_control(&repo)?;
    store.control(id, action)?;
    Ok(json!({"run_id":id,"requested":action}))
}
fn main() {
    let cli = Cli::parse();
    let format = cli.format.unwrap_or(if cli.json {
        Format::Json
    } else {
        Format::Human
    });
    if cli.json && format != Format::Json {
        eprintln!("--json conflicts with --format");
        std::process::exit(2);
    }
    let code = match execute(cli, format) {
        Ok(c) => c,
        Err(e) => {
            let message = format!("{e:#}");
            let code = message
                .split(':')
                .next()
                .filter(|s| s.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
                .unwrap_or("operation_failed");
            let exit = if code.starts_with("invalid")
                || code.contains("selection")
                || code == "operation_failed"
            {
                2
            } else if code.contains("unavailable")
                || code.contains("unsupported")
                || code.contains("configured")
            {
                3
            } else {
                5
            };
            let _ = emit(
                json!({"schema_version":1,"error":{"code":code,"message":message}}),
                format,
            );
            exit
        }
    };
    std::process::exit(code);
}
