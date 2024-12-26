use std::{
    path::{Path, PathBuf},
    sync::{mpsc, LazyLock},
};

use anyhow::{Context, Result};
use notify::Watcher;
use regex::Regex;

use crate::settings::Action;

#[derive(Debug)]
pub struct DustforceAutosplitter {
    split_file: PathBuf,
    split_on_ss: bool,
    last_split: Option<Split>,
    // Need to hold on to the watcher to get events from it.
    #[allow(unused)]
    watcher: notify::INotifyWatcher,
    rx: mpsc::Receiver<notify::Result<notify::Event>>,
}

impl DustforceAutosplitter {
    pub fn new(split_file: PathBuf, split_on_ss: bool) -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx)?;
        watcher.watch(&split_file, notify::RecursiveMode::NonRecursive)?;
        Ok(Self {
            split_file,
            split_on_ss,
            last_split: None,
            watcher,
            rx,
        })
    }

    /// Returns the next currently pending action, or None if there are none.
    pub fn next_action(&mut self) -> Option<Action> {
        while let Ok(result) = self.rx.try_recv() {
            if let Ok(event) = result {
                if let notify::EventKind::Modify(notify::event::ModifyKind::Data(_)) = event.kind {
                    let split = match Split::try_from_path(&self.split_file) {
                        Ok(split) => split,
                        Err(error) => {
                            println!("{error}");
                            continue;
                        }
                    };
                    if (!self.split_on_ss || split.is_ss())
                        && self
                            .last_split
                            .as_ref()
                            .is_none_or(|last_split| *last_split != split)
                    {
                        self.last_split = Some(split);
                        return Some(Action::Split);
                    }
                }
            }
        }
        None
    }
}

static SPLITS_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(\d*)
([^ ]*) (\d*) ([\d\.]*) (\d*)",
    )
    .unwrap()
});

#[derive(Debug, PartialEq)]
struct Split {
    count: u32,
    level: String,
    breaks: u32,
    completion: f32,
    time: u32,
}

impl Split {
    fn try_from_path(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let captures = SPLITS_PATTERN
            .captures(&content)
            .context("could not parse splits.txt")?;
        let (_, [count, level, breaks, completion, time]) = captures.extract();
        let count = count.parse().context("could not parse count")?;
        let breaks = breaks.parse().context("could not parse breaks")?;
        let completion = completion.parse().context("could not parse completion")?;
        let time = time.parse().context("could not parse time")?;
        Ok(Split {
            count,
            level: level.to_string(),
            breaks,
            completion,
            time,
        })
    }

    fn is_ss(&self) -> bool {
        self.breaks == 0 && self.completion == 100.0
    }
}
