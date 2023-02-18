use crate::history::History;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use log::*;
use std::fs::File;
use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    io::{BufRead, BufReader},
    path::PathBuf,
};
use std::{env, os::unix::fs::PermissionsExt};
use tokio::{
    io::{self, AsyncBufReadExt},
    task::{spawn, spawn_blocking},
};

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
    pub fn merge_history(&mut self, history: &History) {
        for entry in history.as_vec().iter() {
            if let Some(elem) = self.inner.iter_mut().find(|x| x.name == entry.name) {
                elem.base_score = entry.num_used;
            } else {
                self.inner.push(Element {
                    name: entry.name.to_owned(),
                    value: entry.value.to_owned(),
                    base_score: entry.num_used,
                })
            }
        }
    }

    pub fn sort_score(&mut self) {
        self.inner.sort_by(|a, b| b.base_score.cmp(&a.base_score))
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

#[derive(Debug, Default)]
pub struct ElementListBuilder {
    from_path: bool,
    from_stdin: bool,
    from_file: Vec<PathBuf>,
}

impl ElementListBuilder {
    pub fn new() -> ElementListBuilder {
        ElementListBuilder::default()
    }

    pub fn add_path(&mut self) {
        self.from_path = true;
    }
    pub fn add_files(&mut self, files: &[PathBuf]) {
        self.from_file = files.to_vec();
    }
    pub fn add_stdin(&mut self) {
        self.from_stdin = true;
    }

    pub async fn build(&self) -> Result<ElementList, Box<dyn std::error::Error>> {
        let mut fut = Vec::new();
        if self.from_stdin {
            fut.push(spawn(ElementListBuilder::build_stdin()))
        }
        if !self.from_file.is_empty() {
            let files = self.from_file.clone();
            fut.push(spawn_blocking(move || {
                ElementListBuilder::build_files(&files)
            }))
        }
        if self.from_path {
            fut.push(spawn_blocking(ElementListBuilder::build_path))
        }

        let finished = futures::future::join_all(fut).await;

        let mut res = Vec::new();
        for elements in finished {
            let mut elements = elements??;
            res.append(&mut elements);
        }

        Ok(ElementList { inner: res })
    }

    fn build_files(files: &[PathBuf]) -> Result<Vec<Element>, std::io::Error> {
        let mut res = Vec::new();
        for file in files {
            let mut reader = BufReader::new(File::open(file)?);
            let mut buf = String::new();
            let mut base_score = 0;

            while reader.read_line(&mut buf)? > 0 {
                let kv_pair = match parse_line(&buf) {
                    None => continue,
                    Some(res) => res,
                };
                match kv_pair {
                    ("%base_score", Some(value)) => {
                        if let Ok(value) = value.parse::<usize>() {
                            base_score = value
                        }
                    }
                    (key, Some(value)) => res.push(Element {
                        name: key.to_string(),
                        value: value.to_string(),
                        base_score,
                    }),
                    ("", None) => {} // Empty Line
                    (key, None) => res.push(Element {
                        name: key.to_string(),
                        value: key.to_string(),
                        base_score,
                    }),
                }

                buf.clear();
            }
        }

        Ok(res)
    }

    fn build_path() -> Result<Vec<Element>, std::io::Error> {
        let var = env::var("PATH").unwrap();

        let mut res: Vec<Element> = Vec::new();

        let paths_iter = env::split_paths(&var);
        let dirs_iter = paths_iter.filter_map(|path| std::fs::read_dir(path).ok());

        for dir in dirs_iter {
            dir.filter_map(|file| file.ok()).for_each(|file| {
                if let Ok(metadata) = file.metadata() {
                    if !metadata.is_dir() && metadata.permissions().mode() & 0o111 != 0 {
                        let name = file.file_name().to_str().unwrap().to_string();
                        res.push(Element {
                            value: name.clone(),
                            name,
                            base_score: 0,
                        });
                    }
                }
            });
        }

        res.sort();
        res.dedup_by(|a, b| a.name == b.name);

        Ok(res)
    }

    async fn build_stdin() -> Result<Vec<Element>, std::io::Error> {
        let stdin = io::stdin();
        let reader = io::BufReader::new(stdin);
        let mut lines = reader.lines();
        let mut res = Vec::new();
        let mut base_score = 0;

        while let Some(line) = lines.next_line().await? {
            let kv_pair = match parse_line(&line) {
                None => continue,
                Some(res) => res,
            };
            match kv_pair {
                ("%base_score", Some(value)) => {
                    if let Ok(value) = value.parse::<usize>() {
                        base_score = value
                    }
                }
                (key, Some(value)) => res.push(Element {
                    name: key.to_string(),
                    value: value.to_string(),
                    base_score,
                }),
                ("", None) => {} // Empty Line
                (key, None) => res.push(Element {
                    name: key.to_string(),
                    value: key.to_string(),
                    base_score,
                }),
            }
        }

        Ok(res)
    }
}

#[allow(clippy::type_complexity)]
fn parse_line(input: &str) -> Option<(&str, Option<&str>)> {
    let input = input.trim();
    let parts = input
        .splitn(2, '=')
        .map(|s| s.trim())
        .collect::<Vec<&str>>();

    if parts.is_empty() {
        warn!("Failed to pares line: {input}");
        None
    } else {
        Some((parts.first().unwrap(), parts.get(1).copied()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_line_test() {
        assert_eq!(parse_line("foobar"), Some(("foobar", None)));
        assert_eq!(parse_line("foo=bar"), Some(("foo", Some("bar"))));
        assert_eq!(
            parse_line("foo=bar\"baz\""),
            Some(("foo", Some("bar\"baz\"")))
        );
        assert_eq!(
            parse_line(
                r#"Desktop: Firefox Developer Edition - New Window=/usr/lib/firefox-developer-edition/firefox --class="firefoxdeveloperedition" --new-window %u"#
            ),
            Some((
                "Desktop: Firefox Developer Edition - New Window",
                Some(
                    r#"/usr/lib/firefox-developer-edition/firefox --class="firefoxdeveloperedition" --new-window %u"#
                )
            ))
        )
    }
}
