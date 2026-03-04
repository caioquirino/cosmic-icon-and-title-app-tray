pub use cosmic::cosmic_config::CosmicConfigEntry;

#[derive(Debug, Clone, cosmic::cosmic_config::cosmic_config_derive::CosmicConfigEntry, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    pub show_all_workspaces: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_all_workspaces: true,
        }
    }
}
