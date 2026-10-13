pub mod containers;
pub mod images;
pub mod logs;
pub mod network;
pub mod overview;
pub mod profiles;
pub mod resources;
pub mod snapshots;
pub mod storage;
pub mod syscheck;

use crate::i18n::{self, Key, Lang};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Overview,
    Containers,
    Resources,
    Network,
    Snapshots,
    Images,
    Profiles,
    Storage,
    Logs,
    SystemCheck,
}

impl Tab {
    pub const ALL: [Tab; 10] = [
        Tab::Overview,
        Tab::Containers,
        Tab::Resources,
        Tab::Network,
        Tab::Snapshots,
        Tab::Images,
        Tab::Profiles,
        Tab::Storage,
        Tab::Logs,
        Tab::SystemCheck,
    ];

    pub fn title(&self, lang: Lang) -> &'static str {
        let key = match self {
            Tab::Overview => Key::TabOverview,
            Tab::Containers => Key::TabContainers,
            Tab::Resources => Key::TabResources,
            Tab::Network => Key::TabNetwork,
            Tab::Snapshots => Key::TabSnapshots,
            Tab::Images => Key::TabImages,
            Tab::Profiles => Key::TabProfiles,
            Tab::Storage => Key::TabStorage,
            Tab::Logs => Key::TabLogs,
            Tab::SystemCheck => Key::TabSystemCheck,
        };
        i18n::t(lang, key)
    }

    pub fn index(&self) -> usize {
        Self::ALL
            .iter()
            .position(|t| t == self)
            .expect("Tab siempre está en Tab::ALL")
    }

    pub fn from_index(index: usize) -> Option<Tab> {
        Self::ALL.get(index).copied()
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}
