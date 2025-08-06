extern crate clap;
use clap::{Arg, Command};

pub fn write_output_file(content: &str) {
    let matches = Command::new("stama")
        .arg(
            Arg::new("output-file")
                .short('o')
                .long("output-file")
                .help("Sets the output file path"),
        )
        .get_matches();

    if let Some(output_file) = matches.get_one::<String>("output-file") {
        std::fs::write(output_file, content).expect("Unable to write file");
    } else {
        println!("It seems that you didn't provide an output file path. The content will be printed to the console instead:");
        println!("{}", content);
        println!("For more information on how to execute the command automatically, please refer to the documentation:");
        println!("GitHub:   https://github.com/Gordi42/stama");
    }
}
