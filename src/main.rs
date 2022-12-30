extern crate chrono;
#[macro_use] extern crate log;

use clap::Parser;
use difflib::sequencematcher::SequenceMatcher;
use std::include_bytes;

#[derive(Parser, Debug)]
#[command(author="Jay Deiman", version, about="", long_about=None)]
struct Args {
    #[arg(short, long, default_value="5")]
    top: usize,
    #[arg(short='D', long)]
    debug: bool,
    #[arg()]
    word: String,
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

fn get_words(fbytes: &'static [u8]) -> Vec<String> {
    let mut ret: Vec<String> = Vec::new();
    let mut buf: Vec<u8> = Vec::new();
    let nl: u8 = b'\n';
    for c in fbytes {
        if c == &nl {
            ret.push(String::from_utf8_lossy(&buf).to_string());
            buf = vec![];
        }
        else {
            buf.push(*c);
        }
    }

    if buf.len() > 0 {
        ret.push(String::from_utf8_lossy(&buf).to_string());
    }

    return ret;
}

fn find_word<'a>(word: &'a str, word_list: &'a Vec<String>) -> Vec<(f32, &'a str)> {
    let mut ret: Vec<(f32, &str)> = Vec::new();

    let mut seq = SequenceMatcher::new(word, &word_list[0]);
    for word in word_list {
        seq.set_second_seq(word);
        ret.push((seq.ratio(), word));
    }
    ret.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    return ret;
}

fn main() {
    let args = get_args();
    setup_logging(&args);
    let fbytes = include_bytes!("../american.txt");
    let words = get_words(fbytes);

    let matches = find_word(&args.word, &words);
    for i in 0..args.top {
        let (ratio, word) = matches[i];
        println!("{}", word);
        if ratio == 1.0 {
            // If we have an exact match, just break out
            break;
        }
    }   
}