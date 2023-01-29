extern crate xdg;

use log::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;
use xdg::BaseDirectories;

use crate::selection::Element;

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

impl History {
    pub fn as_vec(&self) -> &Vec<HistoryEntry> {
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
                return Ok(History {
                    entries: Vec::new(),
                    path: xdg_dirs.place_cache_file("default.csv")?,
                });
            }
        };

        let mut res = History {
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
            entry.value = element.value.to_owned();
        } else {
            self.entries.push(HistoryEntry {
                name: element.name.to_owned(),
                value: element.value.to_owned(),
                num_used: 1,
            })
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
