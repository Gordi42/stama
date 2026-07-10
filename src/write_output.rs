use clap::{value_parser, Arg, Command};
use clap_complete::Shell;

/// Build the clap command definition for stama.
///
/// Kept in a separate function so that it can be reused for
/// generating shell completion scripts (and in tests).
fn build_cli() -> Command {
    Command::new("stama")
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::new("output-file")
                .short('o')
                .long("output-file")
                .value_name("FILE")
                .help("Sets the output file path"),
        )
        .arg(
            Arg::new("completions")
                .long("completions")
                .value_name("SHELL")
                .value_parser(value_parser!(Shell))
                .help("Print a shell completion script to stdout and exit"),
        )
}

/// Parse the command line arguments.
///
/// This is called once at startup (see main.rs), so that
/// `stama --help` (and invalid arguments) exit immediately
/// without starting the terminal user interface. Likewise,
/// `stama --completions <shell>` prints a completion script
/// to stdout and exits before the TUI starts.
///
/// Returns the output file path if one was given with `-o`/`--output-file`.
pub fn parse_args() -> Option<String> {
    let matches = build_cli().get_matches();

    if let Some(shell) = matches.get_one::<Shell>("completions") {
        let mut cmd = build_cli();
        clap_complete::generate(*shell, &mut cmd, "stama", &mut std::io::stdout());
        std::process::exit(0);
    }

    matches.get_one::<String>("output-file").cloned()
}

/// Write the exit command to the output file (if one was given),
/// otherwise print it to the console.
pub fn write_output_file(output_file: Option<&str>, content: &str) {
    if let Some(output_file) = output_file {
        if let Err(err) = std::fs::write(output_file, content) {
            eprintln!("Unable to write output file '{}': {}", output_file, err);
        }
    } else {
        println!("It seems that you didn't provide an output file path. The content will be printed to the console instead:");
        println!("{}", content);
        println!("For more information on how to execute the command automatically, please refer to the documentation:");
        println!("GitHub:   https://github.com/Gordi42/stama");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn cli_definition_is_valid() {
        build_cli().debug_assert();
    }

    #[test]
    fn parses_output_file_short_flag() {
        let matches = build_cli()
            .try_get_matches_from(["stama", "-o", "/tmp/out.txt"])
            .unwrap();
        assert_eq!(
            matches.get_one::<String>("output-file").map(String::as_str),
            Some("/tmp/out.txt")
        );
    }

    #[test]
    fn parses_output_file_long_flag() {
        let matches = build_cli()
            .try_get_matches_from(["stama", "--output-file=/tmp/out.txt"])
            .unwrap();
        assert_eq!(
            matches.get_one::<String>("output-file").map(String::as_str),
            Some("/tmp/out.txt")
        );
    }

    #[test]
    fn parses_completions_shells() {
        for (name, shell) in [
            ("bash", Shell::Bash),
            ("zsh", Shell::Zsh),
            ("fish", Shell::Fish),
        ] {
            let matches = build_cli()
                .try_get_matches_from(["stama", "--completions", name])
                .unwrap();
            assert_eq!(matches.get_one::<Shell>("completions"), Some(&shell));
        }
    }

    #[test]
    fn rejects_unknown_shell() {
        assert!(build_cli()
            .try_get_matches_from(["stama", "--completions", "tcsh"])
            .is_err());
    }

    #[test]
    fn generates_zsh_completion_script() {
        let mut cmd = build_cli();
        let mut buffer = Vec::new();
        clap_complete::generate(Shell::Zsh, &mut cmd, "stama", &mut buffer);
        let script = String::from_utf8(buffer).unwrap();
        assert!(script.starts_with("#compdef stama"));
        assert!(script.contains("--output-file"));
        assert!(script.contains("--completions"));
    }

    #[test]
    fn write_output_file_writes_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.txt");
        let path_str = path.to_str().unwrap();
        write_output_file(Some(path_str), "cd /some/path");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "cd /some/path");
    }
}
