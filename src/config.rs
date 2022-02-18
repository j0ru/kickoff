use crate::color::Color;
use crate::keybinds::{KeyCombo, Modifiers};
use smithay_client_toolkit::seat::keyboard::{keysyms, ModifiersState};
use std::fmt::Debug;
use std::fs::{read_to_string, write};
use std::path::PathBuf;
use xdg::BaseDirectories;

use serde::Deserialize;

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct ColorConfig {
    pub background: Color,
    pub text: Color,
    pub text_query: Color,
    pub text_selected: Color,
    pub prompt: Color,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub delete: Vec<KeyCombo>,
    pub delete_word: Vec<KeyCombo>,
    pub execute: Vec<KeyCombo>,
    pub paste: Vec<KeyCombo>,
    pub complete: Vec<KeyCombo>,
    pub nav_up: Vec<KeyCombo>,
    pub nav_down: Vec<KeyCombo>,
    pub exit: Vec<KeyCombo>,
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct HistoryConfig {
    pub decrease_interval: u64,
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub prompt: String,
    pub padding: u32,
    pub font: Option<String>,
    pub fonts: Vec<String>,
    pub font_size: f32,
    pub colors: ColorConfig,
    pub history: HistoryConfig,
    pub keybindings: KeybindingsConfig,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        KeybindingsConfig {
            delete: vec![
                KeyCombo::new(Modifiers::default(), keysyms::XKB_KEY_BackSpace),
                KeyCombo::new(Modifiers::default(), keysyms::XKB_KEY_Delete),
                KeyCombo::new(Modifiers::default(), keysyms::XKB_KEY_KP_Delete),
            ],
            delete_word: vec![
                KeyCombo::new(
                    ModifiersState {
                        ctrl: true,
                        ..ModifiersState::default()
                    }
                    .into(),
                    keysyms::XKB_KEY_BackSpace,
                ),
                KeyCombo::new(
                    ModifiersState {
                        ctrl: true,
                        ..ModifiersState::default()
                    }
                    .into(),
                    keysyms::XKB_KEY_Delete,
                ),
                KeyCombo::new(
                    ModifiersState {
                        ctrl: true,
                        ..ModifiersState::default()
                    }
                    .into(),
                    keysyms::XKB_KEY_KP_Delete,
                ),
            ],
            execute: vec![
                KeyCombo::new(Modifiers::default(), keysyms::XKB_KEY_Return),
                KeyCombo::new(Modifiers::default(), keysyms::XKB_KEY_KP_Enter),
            ],
            paste: vec![KeyCombo::new(
                ModifiersState {
                    ctrl: true,
                    ..ModifiersState::default()
                }
                .into(),
                keysyms::XKB_KEY_v,
            )],
            complete: vec![KeyCombo::new(Modifiers::default(), keysyms::XKB_KEY_Tab)],
            nav_up: vec![
                KeyCombo::new(Modifiers::default(), keysyms::XKB_KEY_Up),
                KeyCombo::new(Modifiers::default(), keysyms::XKB_KEY_KP_Up),
            ],
            nav_down: vec![
                KeyCombo::new(Modifiers::default(), keysyms::XKB_KEY_Down),
                KeyCombo::new(Modifiers::default(), keysyms::XKB_KEY_KP_Down),
            ],
            exit: vec![KeyCombo::new(Modifiers::default(), keysyms::XKB_KEY_Escape)],
        }
    }
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
            font: None,
            fonts: vec![],
            font_size: 32.,
            colors: ColorConfig::default(),
            history: HistoryConfig::default(),
            keybindings: KeybindingsConfig::default(),
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
    pub fn load(config_path: Option<PathBuf>) -> Result<Self, Box<dyn std::error::Error>> {
        let xdg_dirs = BaseDirectories::with_prefix("kickoff")?;
        if let Some(config_file) = config_path {
            let content = read_to_string(config_file)?;
            Ok(toml::from_str(&content)?)
        } else if let Some(config_file) = xdg_dirs.find_config_file("config.toml") {
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
