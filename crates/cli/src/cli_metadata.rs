//! Read-only CLI grammar bridge for the Commander npm entry point.
use super::{Cli, Format, locale};
use clap::{CommandFactory, Parser, Subcommand};
use serde_json::{Value, json};

#[derive(Subcommand, serde::Serialize)]
pub enum MetadataCommand {
    Describe,
    Parse {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        argv: Vec<String>,
    },
}

fn describe(command: &clap::Command) -> Value {
    let arguments: Vec<_> = command
        .get_arguments()
        .filter(|arg| !["help", "version"].contains(&arg.get_id().as_str()))
        .map(|arg| {
            let range = arg.get_num_args().unwrap_or_default();
            json!({
                "id":arg.get_id().as_str(), "long":arg.get_long(), "short":arg.get_short(),
                "help":arg.get_help().map(ToString::to_string),
                "required":arg.is_required_set(), "global":arg.is_global_set(),
                "hidden":arg.is_hide_set(), "takes_value":arg.get_action().takes_values(),
                "multiple":range.max_values()>1,
                "defaults":arg.get_default_values().iter().map(|v|v.to_string_lossy()).collect::<Vec<_>>(),
                "choices":arg.get_possible_values().iter().filter(|v|!v.is_hide_set()).map(|v|v.get_name()).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "name":command.get_name(), "version":command.get_version(),
        "aliases":command.get_all_aliases().collect::<Vec<_>>(),
        "description":command.get_about().map(ToString::to_string), "hidden":command.is_hide_set(),
        "arguments":arguments,
        "commands":command.get_subcommands().filter(|c|c.get_name()!="cli-metadata" && c.get_name()!="help").map(describe).collect::<Vec<_>>(),
    })
}

pub fn invoke(command: &MetadataCommand, current: locale::Locale) -> Value {
    match command {
        MetadataCommand::Describe => {
            let mut grammar = Cli::command();
            grammar.build();
            let grammar = locale::localize_command(grammar, current);
            json!({"schema_version":1,"locale":current.code(),"data":describe(&grammar)})
        }
        MetadataCommand::Parse { argv } => {
            let current = locale::detect(locale::explicit_from_args(argv).as_deref());
            match Cli::try_parse_from(
                std::iter::once("spec-autonomous").chain(argv.iter().map(String::as_str)),
            ) {
                Ok(cli) if cli.json && cli.format.is_some_and(|f| f != Format::Json) => {
                    json!({"ok":false,"exit_code":2,"stdout":"","stderr":"--json conflicts with --format\n"})
                }
                Ok(cli) => json!({"ok":true,"parsed":cli}),
                Err(error) => {
                    let text = locale::error(current, &error.to_string());
                    json!({"ok":false,"exit_code":error.exit_code(),
                        "stdout":if error.use_stderr(){""}else{&text},
                        "stderr":if error.use_stderr(){&text}else{""}})
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grammar_and_preflight_use_clap_without_loading_a_project() {
        let grammar = invoke(&MetadataCommand::Describe, locale::Locale::En);
        assert!(
            grammar["data"]["commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["name"] == "prepare")
        );
        let args = [
            "--path",
            "/definitely/not/a/project",
            "prepare",
            "--goal",
            "a b $(literal)",
            "--json",
        ];
        let result = invoke(
            &MetadataCommand::Parse {
                argv: args.map(str::to_owned).to_vec(),
            },
            locale::Locale::En,
        );
        assert_eq!(result["ok"], true);
        assert_eq!(result["parsed"]["command"]["name"], "prepare");
        assert_eq!(
            result["parsed"]["command"]["arguments"]["goal"],
            "a b $(literal)"
        );
        assert_eq!(result["parsed"]["json"], true);
    }
    #[test]
    fn invalid_native_values_conflicts_and_help_are_preflight_results() {
        for args in [
            vec!["prepare", "--max-workers", "no"],
            vec!["prepare", "--change", "x", "--feature", "y"],
            vec!["init", "--unknown"],
            vec!["progress", "--json", "--format", "toml"],
        ] {
            let result = invoke(
                &MetadataCommand::Parse {
                    argv: args.into_iter().map(str::to_owned).collect(),
                },
                locale::Locale::En,
            );
            assert_eq!(result["ok"], false);
            assert_eq!(result["exit_code"], 2);
        }
        let result = invoke(
            &MetadataCommand::Parse {
                argv: vec!["prepare".into(), "--help".into()],
            },
            locale::Locale::En,
        );
        assert_eq!(result["exit_code"], 0);
        assert!(result["stdout"].as_str().unwrap().contains("--goal"));
    }
}
