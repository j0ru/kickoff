use crate::color::Color;
use std::fs::{read_to_string, write};
use std::path::PathBuf;
use toml;
use xdg::BaseDirectories;

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(default)]
pub struct ColorConfig {
    pub background: Color,
    pub text: Color,
    pub text_query: Color,
    pub text_selected: Color,
    pub prompt: Color,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct HistoryConfig {
    pub decrease_interval: u64,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Config {
    pub prompt: String,
    pub padding: u32,
    pub font: String,
    pub font_size: f32,
    pub colors: ColorConfig,
    pub history: HistoryConfig,
}
impl Default for ColorConfig {
    fn default() -> Self {
        ColorConfig {
            background: Color(40, 44, 52, 170),
            prompt: Color(171, 178, 191, 255),
            text: Color(255, 255, 255, 255),
            text_query: Color(229, 192, 123, 255),
            text_selected: Color(97, 175, 239, 255),
        }
    }
}
impl Default for Config {
    fn default() -> Self {
        Config {
            prompt: "".to_owned(),
            padding: 100,
            font: "".to_owned(),
            font_size: 32.,
            colors: ColorConfig::default(),
            history: HistoryConfig::default(),
        }
    }
}
impl Default for HistoryConfig {
    fn default() -> Self {
        HistoryConfig {
            decrease_interval: 48,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let xdg_dirs = BaseDirectories::with_prefix("kickoff")?;
        if let Some(config_file) = xdg_dirs.find_config_file("config.toml") {
            let content = read_to_string(config_file)?;
            Ok(toml::from_str(&content)?)
        } else {
            let config_file: PathBuf = xdg_dirs.place_config_file("config.toml")?;
            let default = include_bytes!("../assets/default_config.toml");
            write(config_file, default)?;
            Ok(toml::from_str(&String::from_utf8_lossy(default))?)
        }
    }
}
