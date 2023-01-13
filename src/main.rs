extern crate chrono;
#[macro_use] extern crate log;

use anyhow::Result;
use clap::Parser;
use difflib::sequencematcher::SequenceMatcher;
use std::{
    include_bytes,
    collections::HashSet,
    io::{prelude::*, BufRead, BufReader, Lines},
    path::Path,
    fs::{File, read},
};

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
    /// Search all files recursively
    #[arg(short, long, default_value_t=false)]
    recursive: bool,
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

/// Convert a word_list to a hashset -- destructive
fn to_hashset(word_list: Vec<String>) -> HashSet<String> {
    let mut ret = HashSet::new();

    for word in word_list {
        ret.insert(word);
    }

    return ret;
}

fn read_lines(filename: &str) -> Result<Lines<BufReader<File>>> {
    let file = File::open(&Path::new(filename))?;

    return Ok(BufReader::new(file).lines());
}

/// go through the line and return the words, removing any special chars
fn tokenize(line: &str) -> Vec<String> {
    let mut ret = vec![];
    let mut tmp = String::new();
    for char in line.chars() {
        if char.is_ascii_alphabetic() || char == '-' || char == '\'' {
            // Alphabetic chars, dashes and apostrophes are ok
            if char == '-' || char == '\'' {
                tmp.push(char);
            } else {
                tmp.push(char.to_ascii_lowercase());
            }
        } else {
            // If we get here, we've found a word boundary of some sort,
            // append a copy of the word to our return set
            if tmp.len() > 0 && tmp != "-" && tmp != "'" {
                ret.push(tmp.clone());
            }

            tmp = String::new();
        }
    }

    if tmp.len() > 0 {
        ret.push(tmp);
    }

    return ret;
}

/// Read the file by lines, and output the filename:line number for each
/// misspelled word
fn check_file(
    fname: &str,
    reader: Lines<BufReader<File>>,
    words: &HashSet<String>,
) {
    let mut lcount: u64 = 1;
    for line in reader {
        if let Ok(l) = line {
            let tokens = tokenize(&l);
            for word in &tokens {
                if !words.contains(word) {
                    println!("{}:{} \"{}\"", fname, lcount, word);
                }
            }
        }

        lcount += 1;
    }
}

fn check_files(files: Vec<String>, words: HashSet<String>) {
    for fname in &files {
        let reader = match read_lines(fname) {
            Err(e) => {
                warn!("Failed to open \"{}\" for reading, skipping", fname);
                continue;
            },
            Ok(reader) => reader,
        };

        check_file(fname, reader, &words);
    }
}

fn main() {
    let args = get_args();
    setup_logging(&args);
    let fbytes = include_bytes!("../english.txt");
    let words = get_words(fbytes);

    if args.file {
        // Convert the word list to hashset for fast lookups
        let wset = to_hashset(words);
        check_files(args.word, wset);
    } else {
        let matches = find_word(&args.word[0], &words);
        for i in 0..args.top {
            let (ratio, word) = matches[i];
            println!("{}", word);
            if ratio == 1.0 {
                debug!("Found an exact match for our check");
                // If we have an exact match, just break out
                break;
            }
        }
    }   
}

#[test]
fn test_readlines() {
    let fname = "english.txt";  // This should always be here
    let reader = match read_lines(fname) {
        Err(e) => panic!("Error opening {}: {}", fname, e),
        Ok(reader) => reader,
    };

    let mut lcount: u64 = 1;
    for line in reader {
        if let Err(e) = line {
            panic!("Error reading line {}", lcount);
        }

        lcount += 1;
    }

    println!("Read {} lines", lcount);

    // Test for error on non-existant file
    let fail = read_lines("non-existant file");
    assert!(fail.is_err());
}

#[test]
fn test_tokenize() {
    let test1 = "this is a test";

    // Basic test
    let res = tokenize(test1);
    assert_eq!(res, vec!["this", "is", "a", "test"]);

    // Test special chars
    let test2 = "a hyphen-ated word that's life::monkey";
    let res = tokenize(test2);
    assert_eq!(res, vec!["a", "hyphen-ated", "word", "that's", "life", "monkey"]);

    // Test casing
    let test3 = "A Bad Deal";
    let res = tokenize(test3);
    assert_eq!(res, vec!["a", "bad", "deal"]);
}