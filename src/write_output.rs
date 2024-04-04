extern crate clap;
use clap::{Command, Arg};

pub fn write_output_file(content: &str) {
    let matches = Command::new("stama")
        .arg(Arg::new("output-file")
            .short('o')
            .long("output-file")
            .help("Sets the output file path"))
        .get_matches();

    if let Some(output_file) = matches.get_one::<String>("output-file") {
        std::fs::write(output_file, content).expect("Unable to write file");
    }
}
