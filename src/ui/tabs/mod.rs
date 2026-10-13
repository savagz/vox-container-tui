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

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Containers => "Containers",
            Tab::Resources => "Resources",
            Tab::Network => "Network",
            Tab::Snapshots => "Snapshots",
            Tab::Images => "Images",
            Tab::Profiles => "Profiles",
            Tab::Storage => "Storage",
            Tab::Logs => "Logs",
            Tab::SystemCheck => "System Check",
        }
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
