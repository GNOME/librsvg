//! This crate tests that rsvg-convert's man page fully and properly documents its options.
//! It uses/enforces the format specified in the "OPTIONS" section of `rsvg-convert.rst`.

// Allow references to `mut` statics since there's no multithreading.
//
// If this ever changes and mutable statics are used in any function possibly
// executed in multiple threads, please do the needful (maybe use `RefCell`).
#![allow(static_mut_refs)]

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::iter::Peekable;
use std::mem::MaybeUninit;
use std::ops::RangeInclusive;
use std::process::ExitCode;

use clap::builder::{PossibleValue, ValueRange};
use clap::{Arg, ArgAction};
use regex::Regex;

use self::Error::*;

type UnitResult<E> = Result<(), E>;

// These are statics to avoid recompiling Regex patterns every time the functions in
// which they're used are called, since those functions are called multile times by
// design.
//
// Initialized and dropped in `main()`.
static mut VALUE_NAME_SEGMENT_RE: MaybeUninit<Regex> = MaybeUninit::uninit();
static mut VALUE_NAME_RE: MaybeUninit<Regex> = MaybeUninit::uninit();
static mut POSSIBLE_VALUES_RE: MaybeUninit<Regex> = MaybeUninit::uninit();

#[derive(Debug)]
enum Error {
    InvalidFormat {
        message: String,
        string: String,
        causes: &'static [&'static str],
    },
    UnspecifiedOption(String),

    // Short name
    MismacthedShortNames {
        option: String,
        documented: char,
        specified: char,
    },
    UndocumentedShortName {
        option: String,
        short: char,
    },
    UnspecifiedShortName {
        option: String,
        short: char,
    },

    // Value name
    InvalidValueName {
        option: String,
        value_name: String,
    },
    MismacthedValueNames {
        option: String,
        documented: String,
        specified: String,
    },
    UndocumentedValueName {
        option: String,
        value_name: String,
    },
    UnspecifiedValueName {
        option: String,
        value_name: String,
    },

    // Description
    InvalidDescriptionIndentation {
        option: String,
        line_no: i32,
        indent: String,
        expected: String,
    },
    NoDescription {
        option: String,
    },
    NoValueDescription {
        option: String,
        required_because: &'static str,
    },

    // // Possible values
    InvalidPossibleValues {
        option: String,
        line_range: RangeInclusive<i32>,
        values: Vec<String>,
    },
    MismatchedPossibleValues {
        option: String,
        line_range: RangeInclusive<i32>,
        documented: HashSet<String>,
        specified: HashSet<String>,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvalidFormat {
                message,
                string,
                causes,
            } => {
                f.write_str(message)?;
                write!(f, "\n  string: {string:?}")?;

                if !causes.is_empty() {
                    f.write_str("\n  possible causes:")?;
                    for cause in *causes {
                        write!(f, "\n    - {cause}")?;
                    }
                }
            }
            UnspecifiedOption(option) => write!(f, "`--{option}` is documented but not specified")?,

            // Short name
            MismacthedShortNames {
                option,
                documented,
                specified,
            } => write!(
                f,
                "`--{option}` specifies short name `-{specified}` but documents `-{documented}`",
            )?,
            UndocumentedShortName { option, short } => write!(
                f,
                "`--{option}` specifies short name `-{short}` but documents none",
            )?,
            UnspecifiedShortName { option, short } => write!(
                f,
                "`--{option}` specifies no short name but documents `-{short}`",
            )?,

            // Value name
            InvalidValueName { option, value_name } => {
                write!(f, "Invalid value name {value_name:?} for `--{option}`")?;
                f.write_str(
                    "\
                    \n  possible causes:\
                    \n    - contains a non-alphabetic character other than '-', '.', '_'\
                    \n    - contains any of the allowed non-alphabetic characters in succession\
                    \n    - doesn't start with an alphabetic character\
                    \n    - doesn't end with an alphabetic character",
                )?;
            }
            MismacthedValueNames {
                option,
                documented,
                specified,
            } => write!(
                f,
                "`--{option}` specifies value name {specified:?} but documents {documented:?}",
            )?,
            UndocumentedValueName { option, value_name } => write!(
                f,
                "`--{option}` specifies value name {value_name:?} but documents none",
            )?,
            UnspecifiedValueName { option, value_name } => write!(
                f,
                "`--{option}` specifies no value name but documents {value_name:?}",
            )?,

            // Description
            InvalidDescriptionIndentation {
                option,
                line_no,
                indent,
                expected,
            } => {
                write!(f, "Invalid indentation in the description of `--{option}`")?;
                write!(f, "\n   line no: {line_no}")?;
                write!(f, "\n    indent: {indent:?}")?;
                write!(f, "\n  expected: {expected:?}")?;
            }
            NoDescription { option } => write!(f, "`--{option}` has no description")?,
            NoValueDescription {
                option,
                required_because,
            } => {
                write!(f, "`--{option}` has no value description")?;
                write!(f, "\n  required because {required_because}")?;
            }

            // // Possible values
            InvalidPossibleValues {
                option,
                line_range,
                values,
            } => {
                write!(f, "Invalid possible values {values:?} for `--{option}`")?;
                f.write_str("\n  these values contain whitespace")?;
                write!(f, "\n  possible values documented on lines {line_range:?}")?;
            }
            MismatchedPossibleValues {
                option,
                line_range,
                documented,
                specified,
            } => {
                let undocumented = specified - documented;
                let unspecified = documented - specified;

                write!(f, "Mismatched possible values for `--{option}`")?;
                if !undocumented.is_empty() {
                    write!(f, "\n  specified but not documented: {undocumented:?}")?;
                }
                if !unspecified.is_empty() {
                    write!(f, "\n  documented but not specified: {unspecified:?}")?;
                }
                write!(f, "\n  possible values documented on lines {line_range:?}")?;
            }
        }

        Ok(())
    }
}

fn main() -> ExitCode {
    let command = rsvg_convert::build_cli();
    let mut man_page = BufReader::new(File::open("rsvg-convert.rst").unwrap());
    let mut n_errors = 0;
    let mut options: HashMap<&str, &Arg> = HashMap::new();

    for option in command.get_opts() {
        options.insert(option.get_long().unwrap(), option);
    }

    // Initialize static `Regex`s (see the comment above the static items).
    unsafe {
        VALUE_NAME_SEGMENT_RE.write(Regex::new(r"^\*(.+)\*$").unwrap());
        VALUE_NAME_RE.write(Regex::new(r"(?i)^[a-z]+(?:[-._][a-z]+)*$").unwrap());
        POSSIBLE_VALUES_RE.write(
            Regex::new(r"^Possible values are ((?:\s*``[^`]+``\s*, )+\s*``[^`]+``)\.$").unwrap(),
        );
    }

    if let Err(errors) = check_options(&mut options, &mut man_page) {
        n_errors += errors.len();
        for (line_no, error) in errors {
            eprintln!("line {line_no}: {error}\n");
        }
    }

    if !options.is_empty() {
        n_errors += options.len();
        for long_name in options.keys() {
            eprintln!("`--{long_name}` is specified but not documented\n");
        }
    }

    // Drop static `Regex`s (see the comment above the static items).
    unsafe {
        VALUE_NAME_SEGMENT_RE.assume_init_drop();
        VALUE_NAME_RE.assume_init_drop();
        POSSIBLE_VALUES_RE.assume_init_drop();
    }

    if n_errors == 0 {
        ExitCode::SUCCESS
    } else {
        eprintln!("{n_errors} error(s) occurred.");
        ExitCode::FAILURE
    }
}

fn check_options(
    options: &mut HashMap<&str, &Arg>,
    man_page: &mut BufReader<File>,
) -> UnitResult<Vec<(i32, Error)>> {
    let option_header_re = Regex::new(concat!(
        r"^(?:``-(?<short_name>[a-zA-Z?])``, )?",
        r"``--(?<long_name>[a-z]+(?:-[a-z]+)*)``",
        r"(?: (?<value_names>\S.*?))?\s*$",
    ))
    .unwrap();
    let mut errors: Vec<(i32, Error)> = Vec::new();
    let mut man_page_lines = man_page.lines().map(io::Result::unwrap).zip(1..).peekable();

    for (line, _) in &mut man_page_lines {
        if line == ".. START OF OPTIONS" {
            break;
        }
    }

    while let Some((line, line_no)) = man_page_lines.next() {
        if line == ".. END OF OPTIONS" {
            break;
        }

        if !line.starts_with("``-") {
            continue;
        }

        if !option_header_re.is_match(&line) {
            errors.push((
                line_no,
                InvalidFormat {
                    message: "Invalid option header format".to_string(),
                    string: line,
                    causes: &[
                        "no long name",
                        "no double backquotes around the short and/or long name",
                        "wrong amount of '-' before short and/or long name",
                        "no comma followed by space between the short and long name",
                        "multiple space between the short name and long name",
                        "the short name contains more than one character",
                        "the short name contains a character other than 'a'..'z', 'A'..'Z'",
                        "the long name contains a character other than 'a'..'z', '-'",
                        "no space between the long name and value name",
                        "multiple space between the long name and value name",
                        "value name after short name",
                    ],
                },
            ));
            continue;
        }

        let header = option_header_re.captures(&line).unwrap();
        let long_name = header.name("long_name").unwrap().as_str();

        // Removing so we can easily know what options are undocumented at end.
        if let Some(option) = options.remove(long_name) {
            if let Err(option_errs) = check_option(
                option,
                long_name,
                header
                    .name("short_name")
                    .map(|r#match| r#match.as_str().chars().next().unwrap()),
                header.name("value_names").map(|r#match| r#match.as_str()),
                &mut man_page_lines,
            ) {
                errors.extend(option_errs.into_iter().map(|err| (line_no, err)));
            }
        } else {
            errors.push((line_no, UnspecifiedOption(long_name.to_string())));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_option(
    option: &Arg,
    long_name: &str,
    short_name: Option<char>,
    value_names: Option<&str>,
    man_page_lines: &mut Peekable<impl Iterator<Item = (String, i32)>>,
) -> UnitResult<Vec<Error>> {
    let mut errors: Vec<Error> = Vec::new();
    let value_range = match option.get_num_args() {
        Some(value_range) => value_range,
        None => match option.get_value_names() {
            Some(value_names) => ValueRange::new(value_names.len()),
            None => match option.get_action() {
                ArgAction::Set | ArgAction::Append => ValueRange::SINGLE,
                _ => ValueRange::EMPTY,
            },
        },
    };

    if let Err(error) = check_short_name(option, long_name, short_name) {
        errors.push(*error);
    }
    if let Err(error) = check_value_names(option, long_name, &value_range, value_names) {
        errors.push(*error);
    }
    if let Err(error) = check_description(option, long_name, &value_range, man_page_lines) {
        errors.push(*error);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_short_name(
    option: &Arg,
    long_name: &str,
    short_name: Option<char>,
) -> UnitResult<Box<Error>> {
    if let Some(documented) = short_name {
        if let Some(specified) = option.get_short() {
            if documented != specified {
                return Err(Box::new(MismacthedShortNames {
                    option: long_name.to_string(),
                    documented,
                    specified,
                }));
            }
        } else {
            return Err(Box::new(UnspecifiedShortName {
                option: long_name.to_string(),
                short: documented,
            }));
        }
    } else if let Some(short) = option.get_short() {
        return Err(Box::new(UndocumentedShortName {
            option: long_name.to_string(),
            short,
        }));
    }

    Ok(())
}

// NOTE: Even though this function accepts input for any kind/combination of option
// values, it currently doesn't handle multiple or optional values since `rsvg-convert`
// doesn't use any of such yet. In any such case, this function panics.
//
// If at some point in the future the basis for this is no longer valid, this function
// will need to be refactored.
fn check_value_names(
    option: &Arg,
    long_name: &str,
    value_range: &ValueRange,
    value_names_segment: Option<&str>,
) -> UnitResult<Box<Error>> {
    assert!(
        value_range.max_values() <= 1,
        "Multiple option values are not yet handled: `--{}` takes a maximum of {} values",
        long_name,
        value_range.max_values(),
    );
    assert!(
        value_range.min_values() == value_range.max_values(),
        "Optional option values are not yet handled: `--{}` takes {} optional value(s)",
        long_name,
        value_range.max_values() - value_range.min_values(),
    );

    let value_name = if value_range.takes_values() {
        Some(
            option
                .get_value_names()
                .map_or(option.get_id().as_str(), |value_names| {
                    value_names[0].as_str()
                }),
        )
    } else {
        None
    };

    if let Some(value_names_segment_str) = value_names_segment {
        let documented = get_value_name(long_name, value_names_segment_str)?;

        if let Some(specified) = value_name {
            if documented != specified {
                return Err(Box::new(MismacthedValueNames {
                    option: long_name.to_string(),
                    documented: documented.to_string(),
                    specified: specified.to_string(),
                }));
            }

            let value_name_re = unsafe { VALUE_NAME_RE.assume_init_ref() };

            if !value_name_re.is_match(documented) {
                return Err(Box::new(InvalidValueName {
                    option: long_name.to_string(),
                    value_name: documented.to_string(),
                }));
            }
        } else {
            return Err(Box::new(UnspecifiedValueName {
                option: long_name.to_string(),
                value_name: documented.to_string(),
            }));
        }
    } else if let Some(specified) = value_name {
        return Err(Box::new(UndocumentedValueName {
            option: long_name.to_string(),
            value_name: specified.to_string(),
        }));
    }

    Ok(())
}

fn check_description(
    option: &Arg,
    long_name: &str,
    value_range: &ValueRange,
    man_page_lines: &mut Peekable<impl Iterator<Item = (String, i32)>>,
) -> UnitResult<Box<Error>> {
    let description_lines = get_description_lines(long_name, man_page_lines)?;

    check_description_indentation(long_name, &description_lines)?;

    if value_range.takes_values() {
        // If more description segments get checked at some point, the
        // following statement should be moved outside this block.
        let description_segments = get_description_segments(&description_lines);

        check_value_description(option, long_name, &description_segments)?;
    }

    Ok(())
}

fn check_description_indentation(
    long_name: &str,
    description_lines: &[(String, i32)],
) -> UnitResult<Box<Error>> {
    // First line's indentation.
    let expected: String = description_lines[0]
        .0
        .chars()
        .take_while(char::is_ascii_whitespace)
        .collect();

    for (line, line_no) in &description_lines[1..] {
        if line
            .strip_prefix(&expected)
            // Less, or more indentation -> invalid.
            .map_or(true, |line_indent_stripped| {
                line_indent_stripped
                    .chars()
                    .next()
                    .unwrap()
                    .is_ascii_whitespace()
            })
        {
            return Err(Box::new(InvalidDescriptionIndentation {
                option: long_name.to_string(),
                line_no: *line_no,
                indent: line.chars().take_while(char::is_ascii_whitespace).collect(),
                expected,
            }));
        }
    }

    Ok(())
}

fn check_value_description(
    option: &Arg,
    long_name: &str,
    description_segments: &[(String, RangeInclusive<i32>)],
) -> UnitResult<Box<Error>> {
    let possible_values = option.get_possible_values();

    // Value description is only required for options with a fixed set of possible values.
    if possible_values.is_empty() {
        return Ok(());
    }

    if let Some((value_description, value_desc_range)) = description_segments.get(1) {
        check_possible_values(
            long_name,
            &possible_values,
            value_description,
            value_desc_range,
        )?;
    } else {
        return Err(Box::new(NoValueDescription {
            option: long_name.to_string(),
            required_because: "the option has a fixed set of possible values",
        }));
    }

    Ok(())
}

fn check_possible_values(
    long_name: &str,
    possible_values: &[PossibleValue],
    value_description: &str,
    value_desc_range: &RangeInclusive<i32>,
) -> UnitResult<Box<Error>> {
    let documented = get_possible_values(long_name, value_description, value_desc_range)?;
    let specified: HashSet<&str> = possible_values
        .iter()
        .map(PossibleValue::get_name)
        .collect();

    if documented != specified {
        return Err(Box::new(MismatchedPossibleValues {
            option: long_name.to_string(),
            line_range: value_desc_range.clone(),
            documented: documented.into_iter().map(str::to_string).collect(),
            specified: specified.into_iter().map(str::to_string).collect(),
        }));
    }

    let invalid_values: Vec<&str> = documented
        .into_iter()
        .filter(|value| value.contains(|ch: char| ch.is_ascii_whitespace()))
        .collect();

    if !invalid_values.is_empty() {
        return Err(Box::new(InvalidPossibleValues {
            option: long_name.to_string(),
            line_range: value_desc_range.clone(),
            values: invalid_values.into_iter().map(str::to_string).collect(),
        }));
    }

    Ok(())
}

fn get_description_lines(
    long_name: &str,
    man_page_lines: &mut Peekable<impl Iterator<Item = (String, i32)>>,
) -> Result<Vec<(String, i32)>, Box<Error>> {
    let mut lines: Vec<(String, i32)> = Vec::new();

    while let Some((line, _)) = man_page_lines.peek() {
        // Skip blank/all-whitespace lines.
        if line.chars().all(|ch| ch.is_ascii_whitespace()) {
            man_page_lines.next();
            continue;
        }

        // Description lines must be indented underneath the option header.
        // The first line after the header not fulfiling this criteria marks
        // the end of the description.
        if !line.chars().next().unwrap().is_ascii_whitespace() {
            break;
        }

        lines.push(man_page_lines.next().unwrap());
    }

    if lines.is_empty() {
        Err(Box::new(NoDescription {
            option: long_name.to_string(),
        }))
    } else {
        Ok(lines)
    }
}

fn get_description_segments(
    description_lines: &[(String, i32)],
) -> Vec<(String, RangeInclusive<i32>)> {
    let mut description_lines_iter = description_lines.iter().peekable();
    let mut segments: Vec<(String, RangeInclusive<i32>)> = Vec::new();

    while let Some(&&(_, segment_start)) = description_lines_iter.peek() {
        let mut segment: Vec<&str> = Vec::new();
        // Initialized with the number of the last line in case it doesn't end with '.'.
        let mut segment_end = description_lines.last().unwrap().1;

        for &(ref line, line_no) in &mut description_lines_iter {
            let line_trimmed = line.trim();

            segment.push(line_trimmed);

            if line_trimmed.ends_with(".") {
                segment_end = line_no;
                break;
            }
        }

        segments.push((segment.join(" "), segment_start..=segment_end));
    }

    segments
}

fn get_possible_values<'a>(
    long_name: &str,
    value_description: &'a str,
    value_desc_range: &RangeInclusive<i32>,
) -> Result<HashSet<&'a str>, Box<Error>> {
    let possible_values_re = unsafe { POSSIBLE_VALUES_RE.assume_init_ref() };

    if let Some(possible_values) = possible_values_re.captures(value_description) {
        Ok(possible_values
            .get(1)
            .unwrap()
            .as_str()
            .split(", ")
            .map(|value| {
                value
                    .trim()
                    .strip_prefix("``")
                    .unwrap()
                    .strip_suffix("``")
                    .unwrap()
            })
            .collect())
    } else {
        Err(Box::new(InvalidFormat {
            message: format!(
                "Invalid possible values description (lines {:?}) for option `--{}`",
                value_desc_range, long_name,
            ),
            string: value_description.to_string(),
            causes: &[
                "doesn't start on a new line",
                r#"doesn't start with "Possible values are ""#,
                "no double backquotes around values",
                "no comma followed by space between values",
                "no period '.' at the end of the description",
            ],
        }))
    }
}

fn get_value_name<'a>(long_name: &str, value_name_segment: &'a str) -> Result<&'a str, Box<Error>> {
    let value_name_segment_re = unsafe { VALUE_NAME_SEGMENT_RE.assume_init_ref() };

    if let Some(captures) = value_name_segment_re.captures(value_name_segment) {
        Ok(captures.get(1).unwrap().as_str())
    } else {
        Err(Box::new(InvalidFormat {
            message: format!("Invalid value name segment for option `--{}`", long_name),
            string: value_name_segment.to_string(),
            causes: &["the value name is not sorrounded by asterisks '*'"],
        }))
    }
}
