pub use cosmic::cosmic_config::CosmicConfigEntry;

#[derive(Debug, Clone, cosmic::cosmic_config::cosmic_config_derive::CosmicConfigEntry, serde::Serialize, serde::Deserialize, PartialEq)]
#[version = 1]
pub struct Config {
    pub show_all_workspaces: bool,
    pub context_menu_text_limit: usize,
    pub pinned_apps: Vec<String>,
    pub expand_centered: bool,
    pub item_max_width: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_all_workspaces: false,
            context_menu_text_limit: 25,
            pinned_apps: Vec::new(),
            expand_centered: true,
            item_max_width: 300.0,
        }
    }
}
