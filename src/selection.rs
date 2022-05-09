use crate::history::History;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use std::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd};
use std::error::Error;
use std::{env, os::unix::fs::PermissionsExt};
use tokio::io::{self, AsyncBufReadExt};

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Element {
    pub name: String,
    pub value: String,
    pub base_score: usize,
}

impl Ord for Element {
    fn cmp(&self, other: &Element) -> Ordering {
        match other.base_score.cmp(&self.base_score) {
            Ordering::Equal => self.name.cmp(&other.name),
            e => e,
        }
    }
}

impl PartialOrd for Element {
    fn partial_cmp(&self, other: &Element) -> Option<Ordering> {
        match other.base_score.cmp(&self.base_score) {
            Ordering::Equal => Some(self.name.cmp(&other.name)),
            e => Some(e),
        }
    }
}

#[derive(Debug, Default)]
pub struct ElementList {
    inner: Vec<Element>,
}

impl ElementList {
    pub async fn from_path() -> Result<Self, Box<dyn Error>> {
        match tokio::task::spawn_blocking(ElementList::fetch_list).await? {
            Ok(list) => Ok(list),
            Err(e) => Err(e),
        }
    }

    fn fetch_list() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let var = env::var("PATH")?;

        let mut res = ElementList::default();

        let paths_iter = env::split_paths(&var);
        let dirs_iter = paths_iter.filter_map(|path| std::fs::read_dir(path).ok());

        for dir in dirs_iter {
            let executables_iter = dir.filter_map(|file| file.ok()).filter(|file| {
                if let Ok(metadata) = file.metadata() {
                    return !metadata.is_dir() && metadata.permissions().mode() & 0o111 != 0;
                }
                false
            });

            for exe in executables_iter {
                let name = exe.file_name().to_str().unwrap().to_string();
                res.inner.push(Element {
                    value: name.clone(),
                    name,
                    base_score: 0,
                });
            }
        }

        Ok(res)
    }

    pub async fn from_stdin() -> Result<Self, Box<dyn Error>> {
        // TODO: parse '=' assignments
        let stdin = io::stdin();
        let reader = io::BufReader::new(stdin);
        let mut lines = reader.lines();
        let mut res = ElementList::default();

        while let Some(line) = lines.next_line().await? {
            res.inner.push(Element {
                name: line.clone(),
                value: line.clone(),
                base_score: 0,
            })
        }

        Ok(res)
    }

    pub fn sort(&mut self) {
        self.inner.sort();
    }

    pub fn merge_history(&mut self, history: &History) {
        for entry in history.as_vec().iter() {
            if let Some(elem) = self.inner.iter_mut().find(|x| x.name == entry.name) {
                elem.base_score = entry.num_used;
            } else {
                self.inner.push(Element {
                    name: entry.name.to_owned(),
                    value: entry.name.to_owned(),
                    base_score: entry.num_used,
                })
            }
        }
    }

    pub fn search(&self, pattern: &str) -> Vec<&Element> {
        let matcher = SkimMatcherV2::default();
        let mut executables = self
            .inner
            .iter()
            .map(|x| {
                (
                    matcher
                        .fuzzy_match(&x.name, pattern)
                        .map(|score| score + x.base_score as i64),
                    x,
                )
            })
            .filter(|x| x.0.is_some())
            .collect::<Vec<(Option<i64>, &Element)>>();
        executables.sort_by(|a, b| b.0.unwrap_or(0).cmp(&a.0.unwrap_or(0)));
        executables.into_iter().map(|x| x.1).collect()
    }

    pub fn as_ref_vec(&self) -> Vec<&Element> {
        self.inner.iter().collect()
    }
}
