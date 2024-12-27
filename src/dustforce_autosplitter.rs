use std::{
    path::{Path, PathBuf},
    sync::{mpsc, LazyLock},
};

use anyhow::{Context, Result};
use notify::Watcher;
use regex::Regex;

use crate::settings::Action;

const TUTORIALS: u32 = 0;
const FOREST: u32 = 1;
const MANSION: u32 = 2;
const CITY: u32 = 3;
const LAB: u32 = 4;
const DIFFICULTS: u32 = 5;

const LEVELS_IN_HUB: [u32; 6] = [3, 16, 16, 16, 16, 8];

const BEGINNER_TUTORIAL_FILENAME: &str = "newtutorial1";
const DOWNHILL_FILENAME: &str = "downhill";

#[derive(Debug, Clone, Copy)]
struct RoutePosition {
    hub: u32,
    level: u32,
}

impl RoutePosition {
    fn next(&self) -> Option<Self> {
        if self.level + 1 < LEVELS_IN_HUB[self.hub as usize] {
            Some(Self {
                hub: self.hub,
                level: self.level + 1,
            })
        } else if self.hub != DIFFICULTS {
            Some(Self {
                hub: self.hub + 1,
                level: 0,
            })
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct DustforceAutosplitter {
    split_file: PathBuf,
    split_on_ss: bool,
    split_on_hub: bool,
    route_position: Option<RoutePosition>,
    last_split: Option<Split>,
    // Need to hold on to the watcher to get events from it.
    #[allow(unused)]
    watcher: notify::INotifyWatcher,
    rx: mpsc::Receiver<notify::Result<notify::Event>>,
}

impl DustforceAutosplitter {
    pub fn new(split_file: PathBuf, split_on_ss: bool, split_on_hub: bool) -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx)?;
        watcher.watch(&split_file, notify::RecursiveMode::NonRecursive)?;
        Ok(Self {
            split_file,
            split_on_ss,
            split_on_hub,
            route_position: None,
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
                    let Ok(split) = Split::try_from_path(&self.split_file) else {
                        // Couldn't read the split file, give up until it changes again.
                        continue;
                    };
                    // Check whether anything has changed since the last time we read the file.
                    if self
                        .last_split
                        .as_ref()
                        .is_some_and(|last_split| *last_split == split)
                    {
                        continue;
                    }
                    self.last_split = Some(split.clone());

                    // A level has been completed. Check whether it needs to have been an SS.
                    if !self.split_on_ss || split.is_ss() {
                        // Update where we think we are in the SS All / 16 Reds route.
                        // This assumes that the runner is not using split forest.
                        let old_position = self.route_position;
                        if split.level == BEGINNER_TUTORIAL_FILENAME {
                            self.route_position = Some(RoutePosition {
                                hub: TUTORIALS,
                                level: 1,
                            });
                        } else if split.level == DOWNHILL_FILENAME {
                            self.route_position = Some(RoutePosition {
                                hub: FOREST,
                                level: 1,
                            });
                        } else if let Some(position) = &self.route_position {
                            self.route_position = position.next();
                        }

                        // If we are not splitting on hubs, or we have moved from one hub to the
                        // next, split the timer.
                        if !self.split_on_hub
                            || old_position.is_some_and(|old_position| {
                                self.route_position
                                    .is_none_or(|new_position| old_position.hub + 1 == new_position.hub)
                            })
                        {
                            return Some(Action::Split);
                        }
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

#[derive(Debug, Clone, PartialEq)]
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
