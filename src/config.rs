use std::fs;
use std::path::PathBuf;
use crate::glob::Glob;
use regex::Regex;
use serde::de::Error;
use serde::{Deserialize, Deserializer};
use color_eyre::Result;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub ignored_urls: Vec<Glob>,
    pub ignored_urls_regex: Vec<MyRegex>,
}

#[derive(Debug, Clone)]
pub struct MyRegex(Regex);

impl AsRef<Regex> for MyRegex {
    fn as_ref(&self) -> &Regex {
        &self.0
    }
}

impl<'de> Deserialize<'de> for MyRegex {
    fn deserialize<D>(deserializer: D) -> core::result::Result<MyRegex, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Regex::new(&s).map(MyRegex).map_err(D::Error::custom)
    }
}

pub fn read_app_config() -> Result<Option<AppConfig>> {
    debug_log!("Reading config file: {:?}", PathBuf::from(".").canonicalize()?);
    let file_contents = match fs::read_to_string("config.json") {
        Ok(contents) => {
            if contents.trim().is_empty() {
                return Ok(None);
            }
            Some(contents)
        },
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(e.into());
            }
            None
        }
    };
    let parsed_config = file_contents.map(|it| serde_json::from_str::<AppConfig>(&it)).transpose()?;
    Ok(parsed_config)
}