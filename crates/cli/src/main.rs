use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};
use spec_autonomous_core::{
    capabilities as api, git::Repository, model::HostIdentity, progress, skills, state::Store,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
mod cli_metadata;
mod locale;
#[derive(Parser, serde::Serialize)]
#[command(
    name = "spec-autonomous",
    version,
    about = "Plan, track and deliver OpenSpec / Spec Kit milestones.",
    disable_help_subcommand = true
)]
struct Cli {
    /// Repository directory (also accepted after a subcommand).
    #[arg(long, global = true, default_value = ".")]
    path: PathBuf,
    /// Select the repository's native SDD framework.
    #[arg(long, global = true, value_enum)]
    framework: Option<Framework>,
    /// Emit structured JSON, including errors.
    #[arg(long, global = true)]
    json: bool,
    /// Select the output format for native capabilities.
    #[arg(long, global = true, value_enum)]
    format: Option<Format>,
    /// Choose compact agent output or the full result.
    #[arg(long, global = true, value_enum, default_value = "agent")]
    view: View,
    /// Comma-separated fields to include in structured results.
    #[arg(long, global = true)]
    fields: Option<String>,
    /// Maximum items per result page.
    #[arg(long, global = true)]
    limit: Option<u64>,
    /// Number of items to skip when paging results.
    #[arg(long, global = true)]
    offset: Option<u64>,
    /// Override the detected interface language, for example `zh-CN` or `en-US`.
    #[arg(long, global = true, value_name = "LOCALE")]
    lang: Option<String>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Clone, Copy, ValueEnum, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
enum Framework {
    Auto,
    Openspec,
    Speckit,
}
impl Framework {
    fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Openspec => "openspec",
            Self::Speckit => "speckit",
        }
    }
}
#[derive(Clone, Copy, ValueEnum, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
enum Format {
    Human,
    Json,
    Toml,
}
#[derive(Clone, Copy, ValueEnum, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
enum View {
    Agent,
    Full,
}
#[derive(Clone, Copy, ValueEnum, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    Native,
    Autonomous,
    Plan,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Autonomous => "autonomous",
            Self::Plan => "plan",
        }
    }
}
#[derive(Args, Default, serde::Serialize)]
struct Source {
    #[arg(long,conflicts_with_all=["change","feature"])]
    milestone: Option<String>,
    #[arg(long, conflicts_with = "feature")]
    change: Option<String>,
    #[arg(long)]
    feature: Option<String>,
}
impl Source {
    fn args(self) -> Value {
        let mut v = json!({});
        if let Some(s) = self.milestone {
            v["milestone_id"] = json!(s);
        }
        if let Some(s) = self.change {
            v["change"] = json!(s);
        }
        if let Some(s) = self.feature {
            v["feature"] = json!(s);
        }
        v
    }
}
#[derive(Subcommand, serde::Serialize)]
#[serde(tag = "name", content = "arguments", rename_all = "kebab-case")]
enum Command {
    #[command(hide = true)]
    CliMetadata {
        #[command(subcommand)]
        command: cli_metadata::MetadataCommand,
    },
    /// Inspect native providers, artifacts and structured project state.
    Inspect {
        #[command(flatten)]
        source: Source,
        #[arg(long)]
        phase: Option<String>,
    },
    /// Read progress across every worktree in the common Git repository.
    Progress {
        #[arg(long)]
        all_worktrees: bool,
    },
    /// Prepare work packets and advance the selected milestone.
    #[command(aliases=["run","autonomous","auto"])]
    Prepare {
        #[command(flatten)]
        source: Source,
        #[arg(long,conflicts_with_all=["milestone","change","feature","goal","plan"])]
        run_id: Option<String>,
        #[arg(long,conflicts_with_all=["milestone","change","feature","plan"])]
        goal: Option<String>,
        #[arg(long)]
        id: Option<String>,
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
        max_workers: Option<u64>,
        #[arg(long)]
        delivery: Option<String>,
    },
    /// Preview ready work and blockers without allocating or executing anything.
    Next {
        #[arg(long)]
        run_id: Option<String>,
    },
    /// Accept a host receipt, verify/integrate it and prepare subsequent work.
    ApplyResult {
        #[arg(long)]
        result: PathBuf,
        #[arg(long)]
        token: String,
        #[command(flatten)]
        host: HostArgs,
    },
    /// Preview a native archive; use --apply --plan-hash to execute it.
    Archive {
        #[command(flatten)]
        source: Source,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        plan_hash: Option<String>,
    },
    /// Read configuration, state and repair diagnostics.
    Doctor {
        #[arg(long, hide = true)]
        runner: Option<String>,
    },
    /// Discover or call granular tools through structured JSON arguments.
    Tools {
        #[command(subcommand)]
        command: ToolCommand,
    },
    /// Bind Skills and optionally an owned MCP entry to the existing SDD host.
    Init {
        /// Host profile receiving Skills and optional MCP configuration.
        #[arg(long)]
        agent: Option<String>,
        /// Prefix owned Skill names to avoid an existing name conflict.
        #[arg(long, default_value = "")]
        prefix: String,
        /// Add the project's owned MCP server entry.
        #[arg(long)]
        mcp: bool,
        /// Ask for missing setup choices in the npm terminal interface.
        #[arg(long, conflicts_with_all = ["non_interactive", "yes", "check"])]
        interactive: bool,
        /// Require parameters or detected settings without prompting.
        #[arg(long)]
        non_interactive: bool,
        /// Use OpenSpec, Codex and MCP defaults for missing new-project settings.
        #[arg(long, short = 'y')]
        yes: bool,
        /// Validate owned initialization targets without writing files.
        #[arg(long, hide = true)]
        check: bool,
    },
    /// Serve the same capabilities as stdio MCP tools/resources.
    Mcp {
        #[arg(long)]
        all_tools: bool,
    },
    #[command(hide = true)]
    Detect,
    #[command(hide = true)]
    Skills {
        #[command(subcommand)]
        command: SkillCommand,
    },
    #[command(hide = true)]
    Status { run_id: Option<String> },
    #[command(hide = true)]
    Report { run_id: String },
    #[command(hide = true)]
    Resume {
        run_id: String,
        #[arg(long, value_enum)]
        mode: Option<Mode>,
        #[arg(long)]
        reload_config: bool,
        #[arg(long, default_value_t = 0)]
        extend_seconds: u64,
        #[arg(long)]
        max_attempts: Option<u64>,
    },
    #[command(hide = true)]
    Pause { run_id: String },
    #[command(hide = true)]
    Cancel { run_id: String },
    #[command(hide = true)]
    Cleanup { run_id: String },
    #[command(hide = true)]
    Roadmap {
        #[arg(long)]
        milestone: String,
    },
    #[command(hide = true)]
    Plan {
        #[command(flatten)]
        source: Source,
    },
    #[command(hide = true)]
    Milestone {
        #[command(subcommand)]
        command: MilestoneCommand,
    },
    #[command(hide = true)]
    Claim {
        run_id: String,
        request_id: String,
        #[arg(long)]
        token: String,
        #[command(flatten)]
        host: HostArgs,
    },
    #[command(hide = true)]
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
#[derive(Args, serde::Serialize)]
struct HostArgs {
    #[arg(long)]
    host_id: String,
    #[arg(long)]
    session_id: String,
    #[arg(long)]
    fresh_context: bool,
}
impl HostArgs {
    fn value(self) -> Value {
        json!(HostIdentity {
            host_id: self.host_id,
            session_id: self.session_id,
            fresh_context: self.fresh_context
        })
    }
}
#[derive(Subcommand, serde::Serialize)]
#[serde(tag = "name", content = "arguments", rename_all = "kebab-case")]
enum ToolCommand {
    List {
        #[arg(long)]
        all: bool,
    },
    Call {
        capability: String,
        #[arg(long, default_value = "{}")]
        input: String,
    },
}
#[derive(Subcommand, serde::Serialize)]
#[serde(tag = "name", content = "arguments", rename_all = "kebab-case")]
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
#[derive(Subcommand, serde::Serialize)]
#[serde(tag = "name", content = "arguments", rename_all = "kebab-case")]
enum MilestoneCommand {
    New {
        goal: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, value_enum, default_value = "native")]
        mode: Mode,
        #[arg(long)]
        max_workers: Option<u64>,
    },
}
fn put(v: &mut Value, key: &str, value: Option<impl serde::Serialize>) {
    if let Some(value) = value {
        v[key] = json!(value);
    }
}
fn read_json(path: &Path, limit: u64) -> Result<Value> {
    if fs::metadata(path)?.len() > limit {
        bail!("input_too_large");
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn print(mut value: Value, format: Format, locale: locale::Locale) -> Result<()> {
    progress::portable(&mut value);
    match format {
        Format::Json => println!("{value}"),
        Format::Toml => println!("{}", toml::to_string_pretty(&value)?),
        Format::Human => println!("{}", locale::human(&value, locale)),
    };
    Ok(())
}
fn execute(cli: Cli, format: Format, locale: locale::Locale) -> Result<i32> {
    let root = cli
        .path
        .canonicalize()
        .context("invalid_path: cannot resolve --path")?;
    if !root.is_dir() {
        bail!("invalid_path: expected directory");
    }
    if let Command::Mcp { all_tools } = cli.command {
        return spec_autonomous_core::mcp::serve(
            &root,
            all_tools,
            std::io::BufReader::new(std::io::stdin()),
            std::io::stdout().lock(),
        )
        .map(|_| 0);
    }
    let selected = cli.framework.map(|f| f.name());
    let mut direct = None;
    let (cap, mut args) = match cli.command {
        Command::Inspect { source, phase } => {
            let mut a = source.args();
            put(&mut a, "phase_id", phase);
            ("inspect", a)
        }
        Command::Progress { all_worktrees: _ } => ("progress", json!({})),
        Command::Next { run_id } => {
            let mut a = json!({});
            put(&mut a, "run_id", run_id);
            ("next", a)
        }
        Command::Prepare {
            source,
            run_id,
            goal,
            id,
            plan,
            from,
            to,
            only,
            mode,
            autonomous,
            max_workers,
            delivery,
        } => {
            if autonomous && matches!(mode, Some(Mode::Native)) {
                bail!("invalid_mode: autonomous conflicts with native");
            }
            let mut a = source.args();
            put(&mut a, "run_id", run_id);
            put(&mut a, "goal", goal);
            put(&mut a, "id", id);
            put(&mut a, "from", from);
            put(&mut a, "to", to);
            put(&mut a, "only", only);
            put(&mut a, "max_workers", max_workers);
            put(&mut a, "delivery", delivery);
            put(
                &mut a,
                "mode",
                mode.map(Mode::name)
                    .or(if autonomous { Some("autonomous") } else { None }),
            );
            if let Some(path) = plan {
                let p: spec_autonomous_core::model::Plan =
                    toml::from_str(&fs::read_to_string(path)?)?;
                a["plan"] = json!(p);
            }
            ("prepare", a)
        }
        Command::ApplyResult {
            result,
            token,
            host,
        } => (
            "apply-result",
            json!({"result":read_json(&result,65536)?,"token":token,"host":host.value()}),
        ),
        Command::Archive {
            source,
            apply,
            plan_hash,
        } => {
            let mut a = source.args();
            a["apply"] = json!(apply);
            put(&mut a, "plan_hash", plan_hash);
            ("archive", a)
        }
        Command::Doctor { runner: _ } => ("doctor", json!({})),
        Command::Tools {
            command: ToolCommand::List { all },
        } => ("capabilities", json!({"all":all})),
        Command::Tools {
            command: ToolCommand::Call { capability, input },
        } => {
            let args = if let Some(path) = input.strip_prefix('@') {
                read_json(Path::new(path), 2 * 1024 * 1024)?
            } else {
                serde_json::from_str(&input)?
            };
            return execute_cap(
                &root,
                &capability,
                args,
                &cli.view,
                cli.fields,
                cli.limit,
                cli.offset,
                selected,
                format,
                locale,
            );
        }
        Command::Resume {
            run_id,
            mode,
            reload_config,
            extend_seconds,
            max_attempts,
        } => {
            let mut a = json!({"run_id":run_id,"reload_config":reload_config,"extend_seconds":extend_seconds});
            put(&mut a, "mode", mode.map(Mode::name));
            put(&mut a, "max_attempts", max_attempts);
            ("prepare", a)
        }
        Command::Pause { run_id } => ("run.pause", json!({"run_id":run_id})),
        Command::Cancel { run_id } => ("run.cancel", json!({"run_id":run_id})),
        Command::Cleanup { run_id } => {
            direct = Some(spec_autonomous_core::cleanup::cleanup(&root, &run_id)?);
            ("", json!({}))
        }
        Command::Claim {
            run_id,
            request_id,
            token,
            host,
        } => (
            "work.claim",
            json!({"run_id":run_id,"request_id":request_id,"token":token,"host":host.value()}),
        ),
        Command::ResolveHook {
            run_id,
            key,
            outcome,
            evidence,
        } => (
            "hook.resolve",
            json!({"run_id":run_id,"key":key,"outcome":outcome,"evidence":evidence}),
        ),
        Command::Roadmap { milestone } => ("roadmap.get", json!({"milestone_id":milestone})),
        Command::Plan { source } => {
            let mut a = source.args();
            a["mode"] = json!("plan");
            ("prepare", a)
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
            let mut a = json!({"goal":goal,"mode":mode.name()});
            put(&mut a, "id", id);
            put(&mut a, "max_workers", max_workers);
            ("prepare", a)
        }
        Command::Report { run_id } => ("history.get", json!({"run_id":run_id})),
        Command::Status { run_id } => {
            let repo = Repository::discover(&root)?;
            let store = Store::open(&repo, false)?.context("run_not_found")?;
            let run = if let Some(id) = run_id {
                store.get(&id)?
            } else {
                store.list()?.into_iter().next().context("run_not_found")?
            };
            direct = Some(progress::public_run(&run));
            ("", json!({}))
        }
        Command::Detect => {
            let report = spec_autonomous_core::detect(
                &root,
                match selected {
                    Some("openspec") => Some(spec_autonomous_core::Framework::Openspec),
                    Some("speckit") => Some(spec_autonomous_core::Framework::Speckit),
                    _ => None,
                },
            )?;
            let mut data = serde_json::to_value(&report)?;
            if let Ok(config) = spec_autonomous_core::config::Config::load(&report.root)
                && !config.provider.openspec_command.is_empty()
            {
                data["provider_commands"] = json!({"openspec":config.provider.openspec_command});
            }
            direct = Some(data);
            ("", json!({}))
        }
        Command::Init {
            agent,
            prefix,
            mcp,
            check,
            ..
        } => {
            let project = spec_autonomous_core::detect(&root, None)?.root;
            let framework = match selected {
                Some("openspec") => Some(spec_autonomous_core::Framework::Openspec),
                Some("speckit") => Some(spec_autonomous_core::Framework::Speckit),
                _ => None,
            };
            let data = if check {
                skills::preflight_init(
                    &project,
                    agent.as_deref().unwrap_or("codex"),
                    &prefix,
                    mcp,
                )?;
                json!({"ready":true})
            } else {
                skills::init_with_provider(&project, agent.as_deref(), &prefix, mcp, framework)?
            };
            direct = Some(data);
            ("", json!({}))
        }
        Command::Skills { command } => {
            let project = spec_autonomous_core::detect(&root, None)?.root;
            direct = Some(match command {
                SkillCommand::Install {
                    agent,
                    scope,
                    prefix,
                } => {
                    if scope != "project" {
                        bail!("scope_unsupported");
                    }
                    json!({"installed":skills::install(&project,&agent,&prefix)?})
                }
                SkillCommand::Uninstall => json!({"removed":skills::uninstall(&project)?}),
            });
            ("", json!({}))
        }
        Command::Mcp { .. } | Command::CliMetadata { .. } => unreachable!(),
    };
    if let Some(data) = direct {
        if cap.is_empty() && data.get("detected").is_some() {
            print(data, format, locale)?;
        } else {
            print(json!({"schema_version":1,"data":data}), format, locale)?;
        }
        return Ok(0);
    }
    if cap == "prepare" && args.get("run_id").is_some() {
        args.as_object_mut().unwrap().remove("framework");
    }
    execute_cap(
        &root, cap, args, &cli.view, cli.fields, cli.limit, cli.offset, selected, format, locale,
    )
}
#[allow(clippy::too_many_arguments)]
fn execute_cap(
    root: &Path,
    cap: &str,
    mut args: Value,
    view: &View,
    fields: Option<String>,
    limit: Option<u64>,
    offset: Option<u64>,
    framework: Option<&str>,
    format: Format,
    locale: locale::Locale,
) -> Result<i32> {
    if !args.is_object() {
        bail!("invalid_arguments: input must be a JSON object");
    }
    args["view"] = json!(match view {
        View::Agent => "agent",
        View::Full => "full",
    });
    if let Some(fields) = fields {
        args["fields"] = json!(fields.split(',').map(str::to_string).collect::<Vec<_>>());
    }
    put(&mut args, "limit", limit);
    put(&mut args, "offset", offset);
    if let Some(f) = framework {
        if api::catalog::get(cap)?.input_schema["properties"]
            .get("framework")
            .is_some()
        {
            args["framework"] = json!(f);
        }
    }
    let result = api::invoke(root, cap, &args)?;
    let code = if ["prepare", "apply-result"].contains(&cap) {
        match result["status"].as_str() {
            Some("cancelled") => 130,
            Some("paused" | "needs_input" | "delivery_pending") => 4,
            _ => 0,
        }
    } else {
        0
    };
    print(json!({"schema_version":1,"data":result}), format, locale)?;
    Ok(code)
}
fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let preliminary = locale::detect(locale::explicit_from_args(&raw).as_deref());
    let cli = match locale::parse(&raw, preliminary) {
        Ok(cli) => cli,
        Err(error) => {
            let current = locale::detect(locale::explicit_from_args(&raw).as_deref());
            let text = error.to_string();
            if error.exit_code() == 0 {
                print!("{text}");
            } else if raw.iter().any(|arg| arg == "--json") {
                println!(
                    "{}",
                    json!({"schema_version":1,"error":{"code":"invalid_arguments","message":locale::clap_error(current, &text)}})
                );
            } else {
                eprintln!("{}", locale::clap_error(current, &text));
            }
            std::process::exit(error.exit_code());
        }
    };
    let current_locale = locale::detect(cli.lang.as_deref());
    if let Command::CliMetadata { command } = &cli.command {
        println!("{}", cli_metadata::invoke(command, current_locale));
        return;
    }
    if let Err(error) = ctrlc::set_handler(spec_autonomous_core::process::interrupt) {
        eprintln!("signal_handler_failed: {error}");
        std::process::exit(1);
    }
    let format = cli.format.unwrap_or(if cli.json {
        Format::Json
    } else {
        Format::Human
    });
    if cli.json && format != Format::Json {
        eprintln!("--json conflicts with --format");
        std::process::exit(2);
    }
    let exit = match execute(cli, format, current_locale) {
        Ok(code) => code,
        Err(error) => {
            let raw_message = format!("{error:#}");
            let code = raw_message
                .split(':')
                .next()
                .filter(|s| s.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
                .unwrap_or("operation_failed");
            let message = locale::error(current_locale, &raw_message);
            let exit = if code.starts_with("invalid")
                || code.contains("selection")
                || code == "operation_failed"
            {
                2
            } else if code.contains("unavailable") || code.contains("unsupported") {
                3
            } else {
                5
            };
            let _ = print(
                json!({"schema_version":1,"error":{"code":code,"message":message}}),
                format,
                current_locale,
            );
            exit
        }
    };
    std::process::exit(if spec_autonomous_core::process::interrupted() {
        130
    } else {
        exit
    });
}
