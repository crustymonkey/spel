extern crate chrono;
#[macro_use] extern crate log;

use anyhow::Result;
use clap::Parser;
use difflib::sequencematcher::SequenceMatcher;
#[allow(deprecated)]
use std::{
    include_bytes,
    env::home_dir,
    collections::HashSet,
    io::{BufRead, BufReader, Lines},
    path::PathBuf,
    fs::File,
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

fn read_lines(filename: &PathBuf) -> Result<Lines<BufReader<File>>> {
    let file = File::open(&filename)?;

    return Ok(BufReader::new(file).lines());
}

/// Check that the token actually looks like a word, return true if it looks
/// at least somewhat legit
fn check_token(token: &str) -> bool {
    if token.len() == 0 {
        return false;
    }

    let mut ok = false;
    for c in token.chars() {
        if c.is_ascii_alphanumeric() {
            ok = true;
            break;
        }
    }

    return ok;
}

/// go through the line and return the words, removing any special chars
fn tokenize(line: &str) -> Vec<String> {
    let mut ret = vec![];
    let mut tmp = String::new();
    for c in line.chars() {
        if c.is_ascii_alphabetic() || c == '-' || c == '\'' {
            // Alphabetic chars, dashes and apostrophes are ok
            if c == '-' || c == '\'' {
                tmp.push(c);
            } else {
                tmp.push(c.to_ascii_lowercase());
            }
        } else {
            // If we get here, we've found a word boundary of some sort,
            // append a copy of the word to our return set
            if check_token(&tmp){
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
    fname: &PathBuf,
    reader: Lines<BufReader<File>>,
    words: &HashSet<String>,
    ign_list: &HashSet<String>,
) {
    let mut lcount: u64 = 1;
    for line in reader {
        if let Ok(l) = line {
            let tokens = tokenize(&l);
            for word in &tokens {
                if !words.contains(word) && !ign_list.contains(word) {
                    println!("{}:{} \"{}\"", fname.display(), lcount, word);
                }
            }
        }

        lcount += 1;
    }
}

fn check_files(
    files: &Vec<PathBuf>,
    words: &HashSet<String>,
    ign_list: &HashSet<String>,
) {
    for fpath in files {
        let reader = match read_lines(&fpath) {
            Err(e) => {
                warn!(
                    "Failed to open \"{}\" for reading, skipping: {}",
                    fpath.display(),
                    e
                );
                continue;
            },
            Ok(reader) => reader,
        };

        check_file(fpath, reader, words, ign_list);
    }
}

#[allow(deprecated)]
fn parse_path(fpath: &PathBuf) -> PathBuf {
    if !fpath.starts_with("~") {
        // If it doesn't start with a ~, we just return it
        return fpath.to_owned();
    }

    let mut ret = PathBuf::new();

    // Turn it into a str with the ~ stripped
    let mut path_str = fpath.to_str().unwrap().strip_prefix("~").unwrap();
    if path_str.starts_with("/") {
        path_str = path_str.strip_prefix("/").unwrap();
    }

    ret.push(home_dir().unwrap());
    ret.push(path_str);

    return ret;
}

fn get_ignore_file_contents(fpath: &PathBuf) -> Vec<String> {
    let mut ret: Vec<String> = vec![];

    let real_path = parse_path(fpath);

    if !real_path.exists() {
        // No file, empty vec
        debug!("Ignore file, {}, does not exist", fpath.to_string_lossy());
        return ret;
    }

    let reader = match read_lines(&real_path) {
        Err(e) => {
            warn!(
                "Failed to open \"{}\" for reading ignore content: {}",
                fpath.to_string_lossy(),
                e,
            );
            return ret;
        },
        Ok(r) => r,
    };

    for line in reader {
        if let Ok(l) = line {
            let word = l.trim();
            if word.len() > 0 {
                debug!("Adding '{}' from ignore file", word);
                ret.push(word.to_string());
            }
        }
    }

    return ret;
}

/// Return a list of the ignored words specified on eithe the command-line
/// or via an ignore file
fn get_ignore_list(to_ign: &Option<String>, ign_file: &PathBuf) -> Vec<String> {
    let mut ret = get_ignore_file_contents(ign_file);
    if to_ign.is_none() {
        // If we don't actually have an ignore list, return an empty vec
        return ret;
    }

    let ign = to_ign.as_ref().unwrap();

    let tmp: Vec<String> = ign.split(',').map(
        |w| w.trim().to_string()
    ).collect();

    // Filter empty values
    for item in tmp {
        if item.len() > 0 {
            ret.push(item);
        }
    }

    return ret;
}

fn main() {
    let args = get_args();
    setup_logging(&args);
    let fbytes = include_bytes!("../english.txt");
    let words = get_words(fbytes);

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
    let reader = match read_lines(&PathBuf::from(fname)) {
        Err(e) => panic!("Error opening {}: {}", fname, e),
        Ok(reader) => reader,
    };

    let mut lcount: u64 = 1;
    for line in reader {
        if let Err(e) = line {
            panic!("Error reading line {}: {}", lcount, e);
        }

        lcount += 1;
    }

    println!("Read {} lines", lcount);

    // Test for error on non-existant file
    let fail = read_lines(&PathBuf::from("non-existant file"));
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

#[test]
fn test_get_ignore_list() {
    let s = Some("a,b,c".to_string());

    assert_eq!(
        get_ignore_list(&s, &PathBuf::from("")),
        vec!["a".to_string(), "b".to_string(), "c".to_string()],
    );

    let s2 = Some("a , b  , c,".to_string());
    assert_eq!(
        get_ignore_list(&s2, &PathBuf::from("")),
        vec!["a".to_string(), "b".to_string(), "c".to_string()],
    );

    let s3 = Some("  , ".to_string());
    assert!(get_ignore_list(&s3, &PathBuf::from("")).len() == 0);

    let s4 = None;
    assert!(get_ignore_list(&s4, &PathBuf::from("")).len() == 0);
}

#[test]
fn test_parse_path() {
    let p = PathBuf::from("~/some/file.txt");
    assert_eq!(parse_path(&p), PathBuf::from("/home/jay/some/file.txt"));

    let p = PathBuf::from("some/file.txt");
    assert_eq!(parse_path(&p), p);

    let p = PathBuf::from("/home/jay/some/file.txt");
    assert_eq!(parse_path(&p), p);
}