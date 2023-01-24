
use anyhow::Result;
use difflib::sequencematcher::SequenceMatcher;
use std::{
    env,
    collections::HashSet,
    io::{BufRead, BufReader, Lines, Read},
    path::PathBuf,
    fs::{File},
    vec,
};

/// This processes the dictionary file stored as bytes in the binary itself
pub fn get_words(fbytes: &[u8]) -> Vec<String> {
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

pub fn find_word<'a>(word: &'a str, word_list: &'a Vec<String>) -> Vec<(f32, &'a str)> {
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
pub fn to_hashset(word_list: Vec<String>) -> HashSet<String> {
    let mut ret = HashSet::new();

    for word in word_list {
        ret.insert(word);
    }

    return ret;
}

pub fn read_lines(filename: &PathBuf) -> Result<Lines<BufReader<File>>> {
    let file = File::open(&filename)?;

    return Ok(BufReader::new(file).lines());
}

/// Check that the token actually looks like a word, return true if it looks
/// at least somewhat legit
pub fn check_token(token: &str) -> bool {
    if token.len() == 0 {
        return false;
    }

    let mut ok = false;
    for c in token.chars() {
        if c.is_ascii_alphabetic() {
            ok = true;
            break;
        }
    }

    return ok;
}

pub fn strip_apost(word: &str) -> String {
    let mut ret = word.to_string();
    if ret.ends_with("'s") {
        // Strip off apostrophe s and eval the regular ret
        ret = ret.strip_suffix("'s").unwrap().to_string();
    }

    if ret.ends_with("'") {
        // Strip trailing apostrophes
        ret = ret.strip_suffix("'").unwrap().to_string();
    }

    return ret;
}

/// go through the line and return the words, removing any special chars
pub fn tokenize(line: &str) -> Vec<String> {
    let mut ret = vec![];
    let mut tmp = String::new();
    for c in line.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '\'' {
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
                ret.push(strip_apost(&tmp));
            }

            tmp = String::new();
        }
    }

    if check_token(&tmp) {
        ret.push(strip_apost(&tmp));
    }

    return ret;
}

/// Read the file by lines, and output the filename:line number for each
/// misspelled word
pub fn check_file(
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

pub fn check_files(
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

/// This will basically just handle a ~/, which is silly that I have to
/// do this, but whatever
pub fn parse_path(fpath: &PathBuf) -> PathBuf {
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

    ret.push(env::var("HOME").unwrap());
    ret.push(path_str);

    return ret;
}

pub fn get_ignore_file_contents(fpath: &PathBuf) -> Vec<String> {
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
pub fn get_ignore_list(to_ign: &Option<String>, ign_file: &PathBuf) -> Vec<String> {
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

/// This will spell check words supplied on the command-line
pub fn spell_check_words(
    word_list: &Vec<String>,
    words: Vec<String>,
    top: usize,
    debug: bool,
) {
    let mut topn = top;
    if words.len() < top {
        // Handle the custom word list case where
        topn = words.len();
    }
    debug!("topn: {}", topn);

    for (i, word) in word_list.iter().enumerate() {
        let matches = find_word(word, &words);

        for j in 0..topn {
            let (ratio, word) = matches[j];
            if debug {
                println!("{}: {}", word, ratio);
            } else {
                println!("{}", word);
            }

            if ratio == 1.0 {
                debug!("Found an exact match for our check");
                // If we have an exact match, just break out
                break;
            }
        }

        if i != word_list.len() - 1 {
            println!("\n-----\n");
        }
    }
}

pub fn read_bytes(path: &PathBuf) -> Result<Vec<u8>> {
    let real_path = parse_path(path);
    let mut f = File::open(real_path)?;
    let mut ret = vec![];
    f.read_to_end(&mut ret)?;

    return Ok(ret);
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
    assert_eq!(res, vec!["a", "hyphen-ated", "word", "that", "life", "monkey"]);

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
    if let Ok(home_dir) = env::var("HOME") {
        let p = PathBuf::from("~/some/file.txt");
        assert_eq!(
            parse_path(&p),
            PathBuf::from(&format!("{}/some/file.txt", home_dir))
        );

        let p = PathBuf::from("some/file.txt");
        assert_eq!(parse_path(&p), p);

        let p = PathBuf::from(&format!("{}/some/file.txt", home_dir));
        assert_eq!(parse_path(&p), p);
    }
}

#[test]
fn test_check_token() {
    assert!(check_token("abc"));
    assert!(!check_token("--"));
    assert!(!check_token("1"));
}

#[test]
fn test_get_words() {
    let bytes = b"this\nis\na\nword\n";
    assert_eq!(get_words(bytes), vec!["this", "is", "a", "word"]);

    let bytes = b"a\ndifferent\ntest";
    assert_eq!(get_words(bytes), vec!["a", "different", "test"]);
}

#[test]
fn test_to_hashset() {
    let words = vec!["blah".to_string(), "monkey".to_string()];
    let mut hs = HashSet::new();
    hs.insert("blah".to_string());
    hs.insert("monkey".to_string());
    assert_eq!(to_hashset(words), hs);

    let words = vec![
        "blah".to_string(),
        "monkey".to_string(),
        "blah".to_string(),
    ];
    assert_eq!(to_hashset(words), hs);
}

#[test]
fn test_strip_apost() {
    assert_eq!(strip_apost("jay's"), "jay");
    assert_eq!(strip_apost("players'"), "players");
    assert_eq!(strip_apost("ja'y"), "ja'y");
}

#[test]
fn test_read_bytes() {
    use std::{
        fs::{OpenOptions, remove_file},
        io::Write,
    };

    let fname = PathBuf::from("/tmp/byte_test");
    let bytes = b"This\nis\na\ntest";

    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&fname).unwrap();
        
        f.write_all(bytes).unwrap();
    }
    
    let res = read_bytes(&fname).unwrap();
    assert_eq!(res.as_slice(), bytes);

    if fname.is_file() {
        remove_file(&fname).unwrap();
    }
}