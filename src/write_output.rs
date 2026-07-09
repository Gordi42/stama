use clap::{Arg, Command};

/// Parse the command line arguments.
///
/// This is called once at startup (see main.rs), so that
/// `stama --help` (and invalid arguments) exit immediately
/// without starting the terminal user interface.
///
/// Returns the output file path if one was given with `-o`/`--output-file`.
pub fn parse_args() -> Option<String> {
    let matches = Command::new("stama")
        .arg(
            Arg::new("output-file")
                .short('o')
                .long("output-file")
                .help("Sets the output file path"),
        )
        .get_matches();

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
