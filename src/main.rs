#![feature(iter_intersperse)]

mod parse;
mod program;

use anyhow::Result;
use clap::{App, AppSettings, Arg};
use std::fs::File;
use std::io::{self, BufRead, BufReader};

fn main() -> Result<()> {
    let matches = App::new("saw")
        .arg(
            Arg::with_name("file")
                .short("f")
                .long("file")
                .value_name("FILE")
                .help("Input file")
                .takes_value(true),
        )
        .setting(AppSettings::TrailingVarArg)
        .arg(
            Arg::with_name("prog")
                .index(1)
                .required(true)
                .multiple(true),
        )
        .get_matches();

    let input: Box<dyn BufRead> = match matches.value_of("file") {
        Some(path) => Box::new(BufReader::new(File::open(path)?)),
        None => Box::new(BufReader::new(io::stdin())),
    };

    // required argument, so safe to unwrap
    let commands: Vec<_> = matches.values_of("prog").unwrap().collect();
    let mut program = parse::parse_args(&commands)?;
    input
        .lines()
        .map(|line| {
            match program.run(line?) {
                Some(res) => println!("{}", res),
                _ => (),
            };
            Ok(())
        })
        .collect()
}
