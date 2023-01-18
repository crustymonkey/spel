extern crate chrono;
#[macro_use] extern crate log;

use clap::Parser;
use std::{
    include_bytes,
    path::PathBuf,
};

mod util;
use crate::util::*;

#[derive(Parser, Debug)]
#[command(
    author="Jay Deiman",
    version,
    about="Check spelling",
    long_about="Check the spelling of a word on the command line or check \
        the spelling of the words in a file or files"
)]
struct Args {
    /// The argument(s) here are file(s) instead of a word
    #[arg(short, long, default_value_t=false)]
    file: bool,
    /// A comma-separated list of words to ignore. Only relevant with --file
    #[arg(short, long)]
    ignore: Option<String>,
    /// Ignore list file, this will be added to anything specified with
    /// the --ignore option.  The file should be 1 item (word) per line
    #[arg(short='I', long, default_value="~/.spel_ignore")]
    ignore_file: PathBuf,
    /// When incorrect in a single word check, show the top N possible
    /// correct spellings
    #[arg(short, long, default_value="5")]
    top: usize,
    /// Turn on debug output
    #[arg(short='D', long)]
    debug: bool,
    /// A single word or file or a number of files
    #[arg()]
    word: Vec<String>,
}

static LOGGER: GlobalLogger = GlobalLogger;

struct GlobalLogger;

/// This implements the logging to stderr from the `log` crate
impl log::Log for GlobalLogger {
    fn enabled(&self, meta: &log::Metadata) -> bool {
        return meta.level() <= log::max_level();
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let d = chrono::Local::now();
            eprintln!(
                "{} - {} - {}:{} {} - {}",
                d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                record.level(),
                record.file().unwrap(),
                record.line().unwrap(),
                record.target(),
                record.args(),
            );
        }
    }

    fn flush(&self) {}
}

/// Create a set of CLI args via the `clap` crate and return the matches
fn get_args() -> Args {
    return Args::parse();
}

/// Set the global logger from the `log` crate
fn setup_logging(args: &Args) {
    let l = if args.debug {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(l);
}

fn main() {
    let args = get_args();
    setup_logging(&args);
    let fbytes = include_bytes!("../english.txt");
    let words = get_words(fbytes);

    if args.word.len() == 0 {
        return;
    }

    if args.file {
        // Convert the word list to hashset for fast lookups
        let wset = to_hashset(words);
        let ign_list = to_hashset(
            get_ignore_list(&args.ignore, &args.ignore_file)
        );
        let files: Vec<PathBuf> = args.word.iter().map(
            |f| PathBuf::from(f)
        ).collect();

        check_files(&files, &wset, &ign_list);
    } else {
        spell_check_words(&args.word, words, args.top, args.debug);
    }   
}