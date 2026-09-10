//! Command descriptions shared by help and shell completion output.

use crate::Result;

struct Command {
    name: &'static str,
    usage: &'static str,
    summary: &'static str,
    options: &'static [&'static str],
}

const COMMANDS: &[Command] = &[
    Command {
        name: "init",
        usage: "init DIRECTORY",
        summary: "Create a new Cargo application and metis.json",
        options: &[],
    },
    Command {
        name: "dev",
        usage: "dev MANIFEST [--once|--watch]",
        summary: "Build, run and reload an application during development",
        options: &["--once", "--watch"],
    },
    Command {
        name: "build",
        usage: "build MANIFEST OUTPUT",
        summary: "Build and stage a portable application",
        options: &[],
    },
    Command {
        name: "package",
        usage: "package MANIFEST OUTPUT",
        summary: "Build a portable application and Windows MSI",
        options: &[],
    },
    Command {
        name: "completions",
        usage: "completions SHELL",
        summary: "Generate a shell completion script",
        options: &[],
    },
];

/// Renders the command reference shown by `metis --help`.
pub(crate) fn help() -> String {
    let mut output = String::from(
        "Métis application tooling\n\nUsage:\n  metis <COMMAND> [ARGS]\n\nCommands:\n",
    );
    for command in COMMANDS {
        output.push_str("  ");
        output.push_str(command.usage);
        output.push_str("  ");
        output.push_str(command.summary);
        output.push('\n');
    }
    output.push_str(
        "\nOptions:\n  -h, --help                       Show this help\n\nMANIFEST is a versioned metis.json file. OUTPUT must not exist.\nBuilds use Cargo --locked and compiler artifact messages. The first\nprocess/installer workflow requires Windows; no signing or publication is\nperformed. `dev --watch` reloads after source or resource changes and never\nruns an artifact from a failed build. See the distribution manual for\ninstallation and target limits.\n",
    );
    output
}

/// Generates a completion script from the same command table as [`help`].
pub(crate) fn completions(shell: &str) -> Result<String> {
    match shell.to_ascii_lowercase().as_str() {
        "bash" => Ok(bash()),
        "fish" => Ok(fish()),
        "powershell" | "pwsh" => Ok(powershell()),
        "zsh" => Ok(zsh()),
        _ => {
            Err(format!("unsupported shell {shell:?}; choose bash, fish, powershell or zsh").into())
        }
    }
}

fn command_names() -> String {
    COMMANDS
        .iter()
        .map(|command| format!("'{}'", command.name))
        .collect::<Vec<_>>()
        .join(" ")
}

fn option_names() -> String {
    COMMANDS
        .iter()
        .flat_map(|command| command.options.iter().copied())
        .map(|option| format!("'{option}'"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn bash() -> String {
    format!(
        "_metis() {{\n  local cur prev\n  cur=\"${{COMP_WORDS[COMP_CWORD]}}\"\n  prev=\"${{COMP_WORDS[COMP_CWORD-1]}}\"\n  local commands=({})\n  local options=({})\n  if [[ $COMP_CWORD -eq 1 ]]; then\n    COMPREPLY=( $(compgen -W \"${{commands[*]}}\" -- \"$cur\") )\n  elif [[ $cur == -* ]]; then\n    COMPREPLY=( $(compgen -W \"${{options[*]}}\" -- \"$cur\") )\n  elif [[ $prev == completions ]]; then\n    COMPREPLY=( $(compgen -W \"bash fish powershell zsh\" -- \"$cur\") )\n  fi\n}}\ncomplete -F _metis metis\n",
        command_names(),
        option_names()
    )
}

fn fish() -> String {
    let commands = COMMANDS
        .iter()
        .map(|command| {
            format!(
                "complete -c metis -n '__fish_use_subcommand' -a {} -d '{}'",
                command.name, command.summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{commands}\ncomplete -c metis -n '__fish_seen_subcommand_from completions' -a 'bash fish powershell zsh'\ncomplete -c metis -l help -s h -d 'Show this help'\ncomplete -c metis -n '__fish_seen_subcommand_from dev' -l once -d 'Run one build and exit'\ncomplete -c metis -n '__fish_seen_subcommand_from dev' -l watch -d 'Reload on source or resource changes'\n"
    )
}

fn powershell() -> String {
    format!(
        "Register-ArgumentCompleter -CommandName metis -ScriptBlock {{\n  param($wordToComplete, $commandAst, $cursorPosition)\n  $commands = @({})\n  $options = @({}, '-h', '--help')\n  if ($commandAst.CommandElements.Count -eq 1) {{\n    $commands | Where-Object {{ $_ -like \"$wordToComplete*\" }} | ForEach-Object {{ [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_) }}\n  }} elseif ($wordToComplete.StartsWith('-')) {{\n    $options | Where-Object {{ $_ -like \"$wordToComplete*\" }} | ForEach-Object {{ [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterName', $_) }}\n  }} elseif ($commandAst.CommandElements[1].Value -eq 'completions') {{\n    'bash','fish','powershell','zsh' | Where-Object {{ $_ -like \"$wordToComplete*\" }}\n  }}\n}}\n",
        command_names(),
        option_names()
    )
}

fn zsh() -> String {
    let commands = COMMANDS
        .iter()
        .map(|command| format!("    '{}'['{}']", command.name, command.summary))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "#compdef metis\n_metis() {{\n  local -a commands\n  commands=(\n{commands}\n  )\n  _describe 'command' commands\n}}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_and_completions_share_every_command() {
        let help = help();
        for command in COMMANDS {
            assert!(help.contains(command.name));
        }
        for shell in ["bash", "fish", "powershell", "zsh"] {
            let completion = completions(shell).expect("known shell");
            for command in COMMANDS {
                assert!(
                    completion.contains(command.name),
                    "{shell}: {}",
                    command.name
                );
            }
        }
    }

    #[test]
    fn unknown_completion_shell_is_typed_as_an_error() {
        assert!(completions("tcsh").is_err());
    }
}
