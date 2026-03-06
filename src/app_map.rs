use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesktopAction {
    pub name: String,
    pub exec: String,
}

#[derive(Clone, Debug, Default)]
pub struct AppInfo {
    pub icon: String,
    pub main_exec: Option<String>,
    pub actions: Vec<DesktopAction>,
}

pub fn build_app_map() -> HashMap<String, AppInfo> {
    let mut map = HashMap::new();
    let mut paths = vec![PathBuf::from("/usr/share/applications")];

    if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(home).join(".local/share/applications"));
    }
    paths.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));

    for path in paths {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().map_or(false, |e| e == "desktop") {
                    if let Ok(content) = fs::read_to_string(&p) {
                        parse_desktop_entry(&p, &content, &mut map);
                    }
                }
            }
        }
    }
    map
}

fn parse_desktop_entry(path: &std::path::Path, content: &str, map: &mut HashMap<String, AppInfo>) {
    let mut icon = None;
    let mut wm_class = None;
    let mut main_exec = None;
    let mut actions_list_str = None;
    let mut action_blocks: HashMap<String, DesktopAction> = HashMap::new();
    let mut current_action: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("[Desktop Entry]") {
            current_action = None;
            continue;
        }
        if line.starts_with("[Desktop Action ") && line.ends_with(']') {
            let action_id = line[16..line.len() - 1].to_string();
            current_action = Some(action_id.clone());
            action_blocks.insert(action_id, DesktopAction { name: String::new(), exec: String::new() });
            continue;
        }
        if current_action.is_none() {
            if line.starts_with("Icon=") {
                icon = Some(line[5..].trim().to_string());
            } else if line.starts_with("StartupWMClass=") {
                wm_class = Some(line[15..].trim().to_string());
            } else if line.starts_with("Actions=") {
                actions_list_str = Some(line[8..].trim().to_string());
            } else if line.starts_with("Exec=") {
                main_exec = Some(line[5..].trim().to_string());
            }
        } else if let Some(ref action_id) = current_action {
            if let Some(action) = action_blocks.get_mut(action_id) {
                if line.starts_with("Name=") {
                    action.name = line[5..].trim().to_string();
                } else if line.starts_with("Exec=") {
                    action.exec = line[5..].trim().to_string();
                }
            }
        }
    }

    let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();

    let mut actions = Vec::new();
    if let Some(list_str) = actions_list_str {
        for a_id in list_str.split(';').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            if let Some(ab) = action_blocks.get(a_id) {
                if !ab.name.is_empty() && !ab.exec.is_empty() {
                    actions.push(ab.clone());
                }
            }
        }
    }

    if let Some(i) = icon {
        let app_info = AppInfo { icon: i, main_exec, actions };
        if let Some(w) = wm_class {
            map.insert(w.to_lowercase(), app_info.clone());
        }
        map.insert(filename.to_lowercase(), app_info);
    }
}

/// Resolves an app_id to its AppInfo (icon name + desktop actions).
///
/// Lookup order:
/// 1. Desktop file map (built from .desktop files at startup)
/// 2. Derive a sensible icon name from the app_id itself
///
/// The icon widget's built-in fallback then handles the rest: it tries stripping
/// trailing hyphen-segments from the icon name until something is found in the theme
/// (e.g. "some-app-extra" → "some-app" → "some").
pub fn get_app_info(app_id: &str, map: &HashMap<String, AppInfo>) -> AppInfo {
    if app_id.is_empty() {
        return AppInfo { icon: "application-x-executable-symbolic".to_string(), ..Default::default() };
    }

    let lower_app_id = app_id.to_lowercase();

    if let Some(mapped) = map.get(&lower_app_id) {
        return mapped.clone();
    }

    // Derive a sensible icon name for apps not found in the desktop file map.
    let icon = if app_id.starts_with('/') {
        // Absolute-path app_ids: use the binary name
        app_id.split('/').last().unwrap_or(app_id).to_lowercase()
    } else if lower_app_id.contains('.') {
        // Reverse-DNS app_ids (e.g. "io.github.SomeApp"): use the last segment
        lower_app_id.split('.').last()
            .filter(|s| s.len() > 3)
            .map(|s| s.to_string())
            .unwrap_or(lower_app_id)
    } else {
        lower_app_id
    };

    AppInfo { icon, ..Default::default() }
}
