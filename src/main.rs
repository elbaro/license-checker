#![feature(label_break_value)]
use clap::{App, Arg, SubCommand};
use failure::{format_err, Error};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;
use chrono::Datelike;

fn get_comment<P: AsRef<Path>>(config: &Config, path: P) -> Result<&str, Error> {
    let path = path.as_ref();
    let ext: &str = path
        .extension()
        .expect("no file extension")
        .to_str()
        .expect("not UTF-8 file extension");
    for (_, value) in config.langs.iter() {
        if value.extensions.iter().any(|e| e == ext) {
            return Ok(&value.comment);
        }
    }
    return Err(format_err!("unsupported file extension: {}", ext));
}

fn first_author<P: AsRef<Path>>(path: P) -> Result<String, Error> {
    let path = path.as_ref();
    let repo = git2::Repository::discover(path)?;
    
    let blame = repo.blame_file(path, None)?;
    let mut counter: HashMap<String, usize> = HashMap::new();
    for hunk in blame.iter() {
        let sig = hunk.orig_signature();
        if let Some(name) = sig.name() {
            *counter.entry(name.to_string()).or_default() += 1;
        }
    }
    if counter.is_empty() {
        Err(format_err!("cannot find author info"))
    } else {
        Ok(counter.iter().max_by_key(|(_k,v)| *v).map(|(k,_v)| k).unwrap().to_string())
    }
}

fn lint<P: AsRef<Path>>(config: &Config, path: P) -> Result<(), failure::Error> {
    let path = path.as_ref();
    let comment = get_comment(config, path)?;

    let f = std::fs::File::open(path).expect("Cannot open the source file");
    let f = std::io::BufReader::new(f);
    let mut lines = f.lines().map(|l| l.unwrap()).peekable();

    // shebang
    if lines.peek().expect("file empty").starts_with("#!") {
        lines.next();
        if config.newline_after_shebang {
            let line = lines.peek().expect("no newline after shebang");
            if !line.is_empty() {
                return Err(format_err!("non-empty line after shebang: {}", line));
            }
            lines.next();
        }
    }

    for line in config.template.lines() {
        let file_line = lines
            .next()
            .expect(&format!("Expected: {}\n  Actual: <None>\n", line));
        let line = format!("{} {}", comment, line);
        let template_line = regex::escape(&line)
            .replace("\\{author\\}", "\\w+([ \\t\\w]*\\w)?")
            .replace("\\{year\\}", "\\d{4}");
        let r = regex::Regex::new(&template_line).expect("wrong template");
        if !r.is_match(&file_line) {
            return Err(format_err!(
                "Expected: \"{}\"\n  Actual: \"{}\"",
                template_line,
                file_line
            ));
        }
    }

    if config.newline_after_template {
        if let Some(line) = lines.next() {
            if !line.is_empty() {
                return Err(format_err!(
                    "line after template is not empty: \"{}\"",
                    line
                ));
            }
        }
    }

    Ok(())
}

fn format<P: AsRef<Path>>(config: &Config, path: P) -> Result<(), Error> {
    // case1. first line is #!
    // skip first line. go to case2.
    // case2. first line has no #
    // 2-1. first line is non-empty && config.newline_after_template => insert(template, newline)
    // 2-2. else => insert(template)
    // case3. first line is #
    // no action
    // ask user for safety
    let path = path.as_ref();
    if lint(config, path).is_ok() { return Ok(()); }

    let comment = get_comment(config, path)?;

    let mut buf = std::fs::read_to_string(path)
        .expect(&format!("Cannot open the source file: {}", path.display()));
    let mut loc = 0usize;

    let mut insert_newline_before = 0;
    let mut insert_newline_after = 0;

    // case2
    if buf.starts_with("#!") {
        // skip the shebang line
        if let Some(size) = buf.find('\n') {
            // #! ... \n
            loc = size;
            if config.newline_after_shebang {
                // #! ... \n\n
                if buf[loc..].starts_with('\n') {
                    loc += 1;
                } else {
                    insert_newline_before += 1;
                }
            }
        } else {
            // #! ... EOF
            insert_newline_before += 1;
            loc = buf.len();
            if config.newline_after_shebang {
                insert_newline_before += 1;
            }
        }
    }

    if buf[loc..].starts_with(comment) {
        // case3
        return Err(format_err!(
            "Is this a license comment?\n\"{}..\"",
            &buf[loc..loc + 10]
        ));
    } else {
        // case2
        if config.newline_after_template {
            if !buf[loc..].starts_with('\n') {
                insert_newline_after += 1;
            }
        }
    }

    let mut insertion: String = "".to_string();
    for _ in 0..insert_newline_before {
        insertion += "\n";
    }
    let author = first_author(path)?;
    let year = chrono::Utc::now().year().to_string();
    let license = config
        .template
        .replace("{author}", &author)
        .replace("{year}", &year);
    for line in license.lines() {
        insertion += comment;
        insertion += " ";
        insertion += line;
        insertion += "\n";
    }
    for _ in 0..insert_newline_after {
        insertion += "\n";
    }
    buf.insert_str(loc, &insertion);
    println!("{}", buf);
    Ok(())
}

#[derive(Deserialize)]
struct Config {
    template: String,
    newline_after_shebang: bool,
    newline_after_template: bool,

    #[serde(flatten)]
    langs: HashMap<String, Lang>,
}

#[derive(Deserialize)]
struct Lang {
    extensions: Vec<String>,
    comment: String,
}

fn main() {
    let matches = App::new("License Checker")
        .version("0.1.0")
        .author("Kevin K. <kbknapp@gmail.com>")
        .about("Does awesome things")
        .arg(
            Arg::with_name("config")
                .long("config")
                .help("A path to config toml")
                .takes_value(true)
                .required(true),
        )
        .subcommand(
            SubCommand::with_name("lint")
                .about("Check for the license header in each file")
                .arg(Arg::with_name("path").help("a file path").takes_value(true).required(true))
                .arg(
                    Arg::with_name("quiet")
                        .short("q")
                        .long("quiet")
                        .help("no stdout / stderr"),
                ),
        )
        .subcommand(
            SubCommand::with_name("format")
                .about("Insert a license header in each file")
                .arg(Arg::with_name("path").help("a file path").takes_value(true).required(true))
                .arg(
                    Arg::with_name("quiet")
                        .short("q")
                        .long("quiet")
                        .help("no stdout / stderr"),
                ),
        )
        .get_matches();

    let config_path = matches.value_of("config").unwrap();
    let config_string = std::fs::read_to_string(config_path).expect("cannot read the config file");
    let config: Config = toml::from_str(&config_string).expect("invalid toml");

    // You can handle information about subcommands by requesting their matches by name
    // (as below), requesting just the name used, or both at the same time
    if let Some(matches) = matches.subcommand_matches("lint") {
        let path = matches.value_of("path").unwrap();
        let res = lint(&config, path);
        if res.is_err() {
            if !matches.is_present("quiet") {
                eprintln!("Error in {}\n{}", path, res.unwrap_err());
            }
            std::process::exit(1);
        }
    } else if let Some(matches) = matches.subcommand_matches("format") {
        let path = matches.value_of("path").unwrap();
        let res = format(&config, path);
        if res.is_err() {
            if !matches.is_present("quiet") {
                eprintln!("Error in {}\n{}", path, res.unwrap_err());
            }
            std::process::exit(1);
        }
    } else {
        unreachable!();
    }
}
