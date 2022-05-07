extern crate xdg;

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::PathBuf;
use xdg::BaseDirectories;

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub name: String,
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

    pub async fn load(path: Option<PathBuf>) -> Result<Self, Box<dyn Error>> {
        // TODO: make actually async
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

        let mut rdr = csv::Reader::from_path(history_path).unwrap();
        for result in rdr.deserialize() {
            let record: HistoryEntry = result?;
            res.entries.push(record);
        }

        Ok(res)
    }

    pub fn inc(&mut self, name: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|x| x.name == name) {
            entry.num_used += 1;
        } else {
            self.entries.push(HistoryEntry {
                name: name.to_owned(),
                num_used: 1,
            })
        }
    }

    pub async fn save(&self) -> Result<(), std::io::Error> {
        // TODO: make actually async

        let mut wtr = csv::Writer::from_path(&self.path)?;
        for entry in &self.entries {
            wtr.serialize(entry)?;
        }
        wtr.flush()?;

        Ok(())
    }
}
