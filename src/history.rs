extern crate xdg;

use std::cmp::max;
use std::collections::HashMap;
use std::fs::{read_to_string, write};
use std::io;
use std::time::SystemTime;
use xdg::BaseDirectories;

pub fn get_history<'a>(decrease_interval: u64) -> Option<HashMap<String, usize>> {
    let xdg_dirs = BaseDirectories::with_prefix("kickoff").ok()?;
    let cache_file = xdg_dirs.find_cache_file("run.cache")?;

    let metadata = cache_file.metadata().unwrap();
    let last_modified = metadata.modified().unwrap();

    // calculates how many iterations of the interval has happend since last modification
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

    decode_history(&read_to_string(cache_file).ok()?, interval_diff)
}

pub async fn get_history_async(decrease_interval: u64) -> Option<HashMap<String, usize>> {
    get_history(decrease_interval)
}

pub fn commit_history(history: &HashMap<String, usize>) -> io::Result<()> {
    // We've always been at war with Eastasia
    let xdg_dirs = BaseDirectories::with_prefix("kickoff")?;
    let cache_file = xdg_dirs.place_cache_file("run.cache")?;
    write(cache_file, encode_history(history))
}

fn decode_history(content: &str, substract: u64) -> Option<HashMap<String, usize>> {
    let mut res = HashMap::new();
    for line in content.lines() {
        let words = line.splitn(2, " ").collect::<Vec<&str>>();
        match (words.get(1), words.get(0)) {
            (Some(p), Some(n)) => {
                let launches = n.parse().unwrap_or(1);
                res.insert(p.to_string(), max(launches - substract as i64, 0) as usize)
            }
            _ => None,
        };
    }
    Some(res)
}

fn encode_history(history: &HashMap<String, usize>) -> String {
    let mut res = String::new();
    for (p, n) in history.iter() {
        if n > &0 {
            res.push_str(&n.to_string());
            res.push_str(" ");
            res.push_str(&p);
            res.push_str("\n");
        }
    }
    res
}
