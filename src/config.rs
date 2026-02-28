use crate::glob::Glob;
use color_eyre::Result;
use regex_lite::Regex;
use serde::de::Error;
use serde::{Deserialize, Deserializer};
use std::fs;

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
    let path = if cfg!(debug_assertions) {
        std::env::var("CONFIG_PATH").ok()
    } else {
        None
    }.unwrap_or("config.json".to_owned());

    let file_contents = match fs::read_to_string(&path) {
        Ok(contents) => {
            if contents.trim().is_empty() {
                debug_log!("Config file is empty");
                return Ok(None);
            }
            debug_log!("Config file found: {}", path);
            Some(contents)
        },
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                debug_log!("Error reading config file: {}", e);
                return Err(e.into());
            }
            debug_log!("Config file not found");
            None
        }
    };
    let parsed_config = file_contents.map(|it| serde_json::from_str::<AppConfig>(&it)).transpose()?;
    Ok(parsed_config)
}

pub fn load_env_file() {
    #[cfg(debug_assertions)] {
        dotenvy::from_path_override(".env").ok();
    }
}