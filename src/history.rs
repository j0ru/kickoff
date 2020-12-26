extern crate xdg;

use std::collections::HashMap;
use std::fs::{read_to_string, write};
use std::io;
use xdg::BaseDirectories;

pub fn get_history<'a>() -> Option<HashMap<String, usize>> {
    let xdg_dirs = BaseDirectories::with_prefix("kickoff").ok()?;
    let cache_file = xdg_dirs.find_cache_file("run.cache")?;
    decode_history(&read_to_string(cache_file).ok()?)
}

pub async fn get_history_async() -> Option<HashMap<String, usize>> {
    get_history()
}

pub fn commit_history(history: &HashMap<String, usize>) -> io::Result<()> {
    // We've always been at war with Eastasia
    let xdg_dirs = BaseDirectories::with_prefix("kickoff")?;
    let cache_file = xdg_dirs.place_cache_file("run.cache")?;
    write(cache_file, encode_history(history))
}

fn decode_history(content: &str) -> Option<HashMap<String, usize>> {
    let mut res = HashMap::new();
    for line in content.lines() {
        let words = line.splitn(2, " ").collect::<Vec<&str>>();
        match (words.get(1), words.get(0)) {
            (Some(p), Some(n)) => res.insert(p.to_string(), n.parse().unwrap_or(0)),
            _ => None,
        };
    }
    Some(res)
}

fn encode_history(history: &HashMap<String, usize>) -> String {
    let mut res = String::new();
    for (p, n) in history.iter() {
        res.push_str(&n.to_string());
        res.push_str(" ");
        res.push_str(&p);
        res.push_str("\n");
    }
    res
}
