use crate::color::Color;
use xdg::BaseDirectories;
use toml;
use std::fs::{read_to_string, write};
use std::path::PathBuf;

use serde::{Deserialize};

#[derive(Deserialize)]
pub struct Config {
    pub color_background: Color,
    pub color_text: Color,
    pub color_text_query: Color,
    pub color_text_selected: Color,
    pub color_prompt: Color,
    pub prompt: String,
    pub padding: u32,
    pub font: String,
    pub font_size: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            color_background: Color(40, 44, 52, 170),
            color_prompt: Color(171, 178, 191, 255),
            color_text: Color(255, 255, 255, 255),
            color_text_query: Color(229, 192, 123, 255),
            color_text_selected: Color(97, 175, 239, 255),
            prompt: "".to_owned(),
            padding: 100,
            font: "".to_owned(),
            font_size: 32.,
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
            let default = include_bytes!("default_config.toml");
            write(config_file, default)?;
            Ok(toml::from_str(&String::from_utf8_lossy(default))?)
        }
    }
}
