use color_eyre::eyre::{eyre, ContextCompat};
use color_eyre::Result;
use regex::Regex;
use serde::de::Error;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone)]
pub struct Glob(Regex);

impl Glob {
    pub fn new(glob: &str) -> Result<Self> {
        Ok(Self(glob_to_regex(glob)?))
    }

    pub fn is_match(&self, url: &str) -> bool {
        self.0.is_match(url)
    }
}

impl<'de> Deserialize<'de> for Glob {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Glob, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(&s).map_err(D::Error::custom)
    }
}

const MATCH_ONE_SEGMENT: &str = r"[^\.:/]*?";
const MATCH_ANYTHING: &str = ".*?";

fn glob_to_regex(glob: &str) -> Result<Regex> {
    let protocol_index = glob.find("://")
        .with_context(|| eyre!("Invalid glob '{glob}', missing protocol separator '://'"))?;
    let url_query_params_index = glob.chars().skip(protocol_index + 1)
        .position(|c| c == '?')
        .map(|it| it + protocol_index + 1);

    let mut regex_pattern = String::with_capacity(glob.len() * 2);
    regex_pattern.push_str("(?i)^");
    let mut index = 0;

    while index < glob.len() {
        let current = glob.chars().nth(index).unwrap();
        let next = glob.chars().nth(index + 1);

        match (current, next) {
            ('/', _) if url_query_params_index.is_none() || Some(index + 1) == url_query_params_index => {
                regex_pattern.push_str("/?");
            }
            ('*', Some('*')) => {
                let pattern = if index < protocol_index  {
                    MATCH_ONE_SEGMENT // We are in the start of the URL, match only the protocol
                } else {
                    MATCH_ANYTHING
                };
                regex_pattern.push_str(pattern);
                index += 1;
            },
            ('*', _) => {
                let pattern = if url_query_params_index.filter(|&it| index > it).is_some() {
                    MATCH_ANYTHING // We are in the query params, match everything until the end
                } else {
                    MATCH_ONE_SEGMENT
                };
                regex_pattern.push_str(pattern)
            },
            _ => {
                if is_regex_meta_character(current) {
                    regex_pattern.push('\\');
                }
                regex_pattern.push(current);
            }
        }
        index += 1;
    }
    if url_query_params_index.is_none() && regex_pattern.ends_with('/') {
        regex_pattern.pop();
    }
    if url_query_params_index.is_none() {
        regex_pattern.push_str("/?$");
    }
    regex_pattern.push('$');

    Ok(Regex::new(&regex_pattern)?)
}

fn is_regex_meta_character(c: char) -> bool {
    match c {
        '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{'
        | '}' | '^' | '$' | '#' | '&' | '-' | '~' => true,
        _ => false,
    }
}