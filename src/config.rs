use crate::color::Color;
use crate::keybinds::{KeyCombo, Modifiers};
use crate::selection::Element;
use log::info;
use smithay_client_toolkit::seat::keyboard::{Keysym, Modifiers as ModifiersState};
use std::fmt::Debug;
use std::fs::{read_to_string, write};
use std::path::PathBuf;
use std::time::SystemTime;
use xdg::BaseDirectories;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Debug)]
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

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct HistoryConfig {
    pub decrease_interval: u64,
}

#[derive(Deserialize, Clone, Debug)]
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
        Self {
            delete: vec![
                KeyCombo::new(Modifiers::default(), Keysym::BackSpace),
                KeyCombo::new(Modifiers::default(), Keysym::Delete),
                KeyCombo::new(Modifiers::default(), Keysym::KP_Delete),
            ],
            delete_word: vec![
                KeyCombo::new(
                    ModifiersState {
                        ctrl: true,
                        ..ModifiersState::default()
                    }
                    .into(),
                    Keysym::BackSpace,
                ),
                KeyCombo::new(
                    ModifiersState {
                        ctrl: true,
                        ..ModifiersState::default()
                    }
                    .into(),
                    Keysym::Delete,
                ),
                KeyCombo::new(
                    ModifiersState {
                        ctrl: true,
                        ..ModifiersState::default()
                    }
                    .into(),
                    Keysym::KP_Delete,
                ),
            ],
            execute: vec![
                KeyCombo::new(Modifiers::default(), Keysym::Return),
                KeyCombo::new(Modifiers::default(), Keysym::KP_Enter),
            ],
            paste: vec![KeyCombo::new(
                ModifiersState {
                    ctrl: true,
                    ..ModifiersState::default()
                }
                .into(),
                Keysym::v,
            )],
            complete: vec![KeyCombo::new(Modifiers::default(), Keysym::Tab)],
            nav_up: vec![
                KeyCombo::new(Modifiers::default(), Keysym::Up),
                KeyCombo::new(Modifiers::default(), Keysym::KP_Up),
            ],
            nav_down: vec![
                KeyCombo::new(Modifiers::default(), Keysym::Down),
                KeyCombo::new(Modifiers::default(), Keysym::KP_Down),
            ],
            exit: vec![KeyCombo::new(Modifiers::default(), Keysym::Escape)],
        }
    }
}
impl Default for ColorConfig {
    fn default() -> Self {
        Self {
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
        Self {
            prompt: String::new(),
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
        Self {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub name: String,
    pub value: String,
    pub num_used: usize,
}

#[derive(Debug)]
pub struct History {
    entries: Vec<HistoryEntry>,
    path: PathBuf,
}

impl Default for History {
    fn default() -> Self {
        let xdg_dirs = BaseDirectories::with_prefix("kickoff")
            .expect("Failed to determine xdg base directory");
        Self {
            entries: Vec::new(),
            path: xdg_dirs
                .place_cache_file("default.csv")
                .expect("Failed to place history file"),
        }
    }
}

impl History {
    pub const fn as_vec(&self) -> &Vec<HistoryEntry> {
        &self.entries
    }

    pub fn load(path: Option<PathBuf>, decrease_interval: u64) -> Result<Self, std::io::Error> {
        let history_path = if let Some(path) = path {
            path
        } else {
            let xdg_dirs = BaseDirectories::with_prefix("kickoff")?;
            if let Some(path) = xdg_dirs.find_cache_file("default.csv") {
                path
            } else {
                return Ok(Self {
                    entries: Vec::new(),
                    path: xdg_dirs.place_cache_file("default.csv")?,
                });
            }
        };

        let mut res = Self {
            entries: Vec::new(),
            path: history_path.clone(),
        };

        if history_path.exists() {
            let last_modified = history_path.metadata()?.modified()?;
            let interval_diff = if decrease_interval > 0 {
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    / (3600 * decrease_interval)
                    - last_modified
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        / (3600 * decrease_interval)
            } else {
                0
            };

            let mut rdr = csv::Reader::from_path(history_path).unwrap();
            for result in rdr.deserialize() {
                let mut record: HistoryEntry = result?;
                record.num_used = record.num_used.saturating_sub(interval_diff as usize);
                if record.num_used > 0 {
                    res.entries.push(record);
                }
            }
        } else {
            info!("History file does not exists, will be created on saving");
        }

        Ok(res)
    }

    pub fn inc(&mut self, element: &Element) {
        if let Some(entry) = self.entries.iter_mut().find(|x| x.name == element.name) {
            entry.num_used += 1;
            entry.value = element.value.clone();
        } else {
            self.entries.push(HistoryEntry {
                name: element.name.clone(),
                value: element.value.clone(),
                num_used: 1,
            });
        }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let mut wtr = csv::Writer::from_path(&self.path)?;
        for entry in &self.entries {
            wtr.serialize(entry)?;
        }
        wtr.flush()?;

        Ok(())
    }
}
