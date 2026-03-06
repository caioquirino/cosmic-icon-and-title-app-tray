mod config;

use cosmic_applet_window_list::wayland_subscription::{
    wayland_subscription, ToplevelUpdate, WaylandRequest, WaylandUpdate, ToplevelRequest,
};
use cctk::toplevel_info::ToplevelInfo;
use cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_handle_v1::State;
use cctk::wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1;
use cctk::wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1::ExtWorkspaceHandleV1;
use cosmic::app::{Core};
use cosmic::iced::{Alignment, Length, Subscription, Limits, window, Background, Color, Border};
use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::widget::{self};
use cosmic::{Element, Task};
use tracing_subscriber::EnvFilter;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::config::Config;
use cosmic::cosmic_config::CosmicConfigEntry;

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

struct WindowListApplet {
    core: Core,
    windows: Vec<(usize, ExtForeignToplevelHandleV1, ToplevelInfo)>,
    next_id: usize,
    active_workspaces: Vec<ExtWorkspaceHandleV1>,
    wayland_tx: Option<cctk::sctk::reexports::calloop::channel::Sender<WaylandRequest>>,
    connection_finished: bool,
    app_map: HashMap<String, AppInfo>,
    config: Config,
    config_handler: cosmic::cosmic_config::Config,
}

#[derive(Debug, Clone)]
pub enum Message {
    WaylandUpdate(WaylandUpdate),
    Activate(usize),
    Close(usize),
    CloseAll(usize),
    ActivateWindow(usize),
    Pin(usize),
    Spawn(usize, usize),
    AppMapLoaded(HashMap<String, AppInfo>),
    ConfigChanged(Config),
    LaunchApp(String),
    LaunchPinned(usize),
    UnpinPinned(usize),
    OpenSettings,
    OpenAbout,
    SurfaceAction(cosmic::surface::Action),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppAction {
    About,
    Settings,
}

impl cosmic::widget::menu::action::MenuAction for AppAction {
    type Message = Message;
    fn message(&self) -> Self::Message {
        match self {
            AppAction::About => Message::OpenAbout,
            AppAction::Settings => Message::OpenSettings,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowAction {
    Close(usize),
    CloseAll(usize),
    ActivateWindow(usize),
    Pin(usize),
    Spawn(usize, usize),
    LaunchPinned(usize),
    UnpinPinned(usize),
}

impl cosmic::widget::menu::action::MenuAction for WindowAction {
    type Message = Message;
    fn message(&self) -> Self::Message {
        match self {
            WindowAction::Close(id) => Message::Close(*id),
            WindowAction::CloseAll(id) => Message::CloseAll(*id),
            WindowAction::ActivateWindow(id) => Message::Activate(*id),
            WindowAction::Pin(id) => Message::Pin(*id),
            WindowAction::Spawn(win_id, act_idx) => Message::Spawn(*win_id, *act_idx),
            WindowAction::LaunchPinned(idx) => Message::LaunchPinned(*idx),
            WindowAction::UnpinPinned(idx) => Message::UnpinPinned(*idx),
        }
    }
}

fn build_app_map() -> HashMap<String, AppInfo> {
    let mut map = HashMap::new();
    let mut paths = vec![
        PathBuf::from("/usr/share/applications"),
    ];
    
    if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(home).join(".local/share/applications"));
    }

    // Flatpak
    paths.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));

    for path in paths {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().map_or(false, |e| e == "desktop") {
                    if let Ok(content) = fs::read_to_string(&p) {
                        let mut icon = None;
                        let mut wm_class = None;
                        let mut main_exec = None;
                        let mut actions_list_str = None;
                        let mut action_blocks = HashMap::new();
                        
                        let mut current_action = None;

                        for line in content.lines() {
                            let line = line.trim();
                            if line.starts_with("[Desktop Entry]") {
                                current_action = None;
                                continue;
                            }
                            if line.starts_with("[Desktop Action ") && line.ends_with(']') {
                                let action_id = line[16..line.len()-1].to_string();
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
                        
                        let filename = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
                        
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
                }
            }
        }
    }
    map
}

fn get_app_info(app_id: &str, title: &str, map: &HashMap<String, AppInfo>) -> AppInfo {
    let lower_app_id = app_id.to_lowercase();
    let lower_title = title.to_lowercase();

    if app_id.is_empty() {
        if lower_title.contains("intellij") || lower_title.contains("idea") { return AppInfo { icon: "intellij-idea".to_string(), ..Default::default() }; }
        if lower_title.contains("zen") { return AppInfo { icon: "zen-browser".to_string(), ..Default::default() }; }
        if lower_title.contains("settings") { return AppInfo { icon: "com.system76.CosmicSettings".to_string(), ..Default::default() }; }
        return AppInfo { icon: "application-x-executable-symbolic".to_string(), ..Default::default() };
    }

    if let Some(mapped) = map.get(&lower_app_id) {
        return mapped.clone();
    }

    // fallback icon matching...
    let mut fallback_icon = lower_app_id.clone();
    if lower_app_id.contains("cosmic-settings") || lower_title.contains("cosmic settings") {
        fallback_icon = "com.system76.CosmicSettings".to_string();
    } else if lower_app_id.contains("idea") || lower_app_id.contains("intellij") || lower_title.contains("intellij") { 
        fallback_icon = "intellij-idea".to_string(); 
    } else if lower_app_id == "zen" || lower_app_id.contains("zen-browser") || lower_title.contains("zen browser") { 
        fallback_icon = "zen-browser".to_string(); 
    } else if lower_app_id == "alacritty" { 
        fallback_icon = "alacritty".to_string(); 
    } else if app_id.starts_with('/') {
        if let Some(name) = app_id.split('/').last() {
            fallback_icon = name.to_lowercase();
        }
    } else if lower_app_id.contains('.') {
        if let Some(last) = lower_app_id.split('.').last() {
            if last.len() > 3 { fallback_icon = last.to_string(); }
        }
    }

    AppInfo { icon: fallback_icon, ..Default::default() }
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.chars().count() > max_len {
        let mut truncated: String = text.chars().take(max_len).collect();
        truncated.push_str("...");
        truncated
    } else {
        text.to_string()
    }
}

fn win11_button_style() -> cosmic::theme::Button {
    cosmic::theme::Button::Custom {
        active: Box::new(move |_focused, theme| {
            let cosmic = theme.cosmic();
            widget::button::Style {
                background: None,
                text_color: Some(Color::from(cosmic.on_bg_color())),
                border_radius: 4.0.into(),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                ..Default::default()
            }
        }),
        hovered: Box::new(move |_focused, theme| {
            let cosmic = theme.cosmic();
            widget::button::Style {
                background: Some(Background::Color(Color::from(cosmic.background.component.hover))),
                text_color: Some(Color::from(cosmic.on_bg_color())),
                border_radius: 4.0.into(),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                ..Default::default()
            }
        }),
        disabled: Box::new(|_theme| Default::default()),
        pressed: Box::new(move |_focused, theme| {
            let cosmic = theme.cosmic();
            widget::button::Style {
                background: Some(Background::Color(Color::from(cosmic.background.component.pressed))),
                text_color: Some(Color::from(cosmic.on_bg_color())),
                border_radius: 4.0.into(),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                ..Default::default()
            }
        }),
    }
}

impl cosmic::Application for WindowListApplet {
    type Executor = cosmic::executor::Default;
    type Message = Message;
    type Flags = ();

    const APP_ID: &'static str = "io.github.caioquirino.CosmicWindowList";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let config_handler = cosmic::cosmic_config::Config::new(Self::APP_ID, 1).unwrap();
        let config = Config::get_entry(&config_handler).ok().unwrap_or_default();
        (
            WindowListApplet {
                core,
                windows: Vec::new(),
                next_id: 0,
                active_workspaces: Vec::new(),
                wayland_tx: None,
                connection_finished: false,
                app_map: HashMap::new(),
                config,
                config_handler,
            },
            Task::perform(async { build_app_map() }, |m| cosmic::Action::App(Message::AppMapLoaded(m))),
        )
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::WaylandUpdate(update) => match update {
                WaylandUpdate::Init(tx) => {
                    self.wayland_tx = Some(tx);
                    self.connection_finished = false;
                }
                WaylandUpdate::Toplevel(toplevel_update) => match toplevel_update {
                    ToplevelUpdate::Add(info, handle) => {
                        if !self.windows.iter().any(|(_, h, _)| h == &handle) {
                            self.windows.push((self.next_id, handle, info));
                            self.next_id += 1;
                        }
                    }
                    ToplevelUpdate::Update(info, handle) => {
                        if let Some(pos) = self.windows.iter().position(|(_, h, _)| h == &handle) {
                            self.windows[pos].2 = info;
                        } else {
                            self.windows.push((self.next_id, handle, info));
                            self.next_id += 1;
                        }
                    }
                    ToplevelUpdate::Remove(handle) => {
                        self.windows.retain(|(_, h, _)| h != &handle);
                    }
                },
                WaylandUpdate::Workspace(active) => {
                    self.active_workspaces = active;
                }
                WaylandUpdate::Finished => {
                    self.connection_finished = true;
                    self.windows.clear();
                }
            },
            Message::Activate(id) | Message::ActivateWindow(id) => {
                if let Some(handle) = self.windows.iter().find(|(i, _, _)| *i == id).map(|(_, h, _)| h) {
                    if let Some(tx) = self.wayland_tx.as_ref() {
                        let _ = tx.send(WaylandRequest::Toplevel(ToplevelRequest::Activate(handle.clone())));
                    }
                }
            }
            Message::Close(id) => {
                if let Some(handle) = self.windows.iter().find(|(i, _, _)| *i == id).map(|(_, h, _)| h) {
                    if let Some(tx) = self.wayland_tx.as_ref() {
                        let _ = tx.send(WaylandRequest::Toplevel(ToplevelRequest::Quit(handle.clone())));
                    }
                }
            }
            Message::CloseAll(id) => {
                if let Some(app_id) = self.windows.iter().find(|(i, _, _)| *i == id).map(|(_, _, info)| info.app_id.clone()) {
                    let handles: Vec<_> = self.windows.iter()
                        .filter(|(_, _, info)| info.app_id == app_id)
                        .map(|(_, handle, _)| handle.clone())
                        .collect();
                    
                    if let Some(tx) = self.wayland_tx.as_ref() {
                        for handle in handles {
                            let _ = tx.send(WaylandRequest::Toplevel(ToplevelRequest::Quit(handle)));
                        }
                    }
                }
            }
            Message::Pin(id) => {
                if let Some(app_id) = self.windows.iter().find(|(i, _, _)| *i == id).map(|(_, _, info)| info.app_id.clone()) {
                    if self.config.pinned_apps.contains(&app_id) {
                        self.config.pinned_apps.retain(|a| a != &app_id);
                    } else {
                        self.config.pinned_apps.push(app_id.clone());
                    }
                    let _ = self.config.write_entry(&self.config_handler);
                }
            }
            Message::LaunchApp(app_id) => {
                let lower_id = app_id.to_lowercase();
                if let Some(info) = self.app_map.get(&lower_id) {
                    if let Some(ref exec) = info.main_exec {
                        let exec = exec.replace("%u", "").replace("%U", "").replace("%f", "").replace("%F", "");
                        let _ = std::process::Command::new("sh").arg("-c").arg(&exec).spawn();
                    }
                }
            }
            Message::Spawn(window_id, action_idx) => {
                if let Some(info) = self.windows.iter().find(|(i, _, _)| *i == window_id).map(|(_, _, info)| info) {
                    let app_info = get_app_info(&info.app_id, &info.title, &self.app_map);
                    if let Some(action) = app_info.actions.get(action_idx) {
                        let exec = action.exec.replace("%u", "").replace("%U", "").replace("%f", "").replace("%F", "");
                        let _ = std::process::Command::new("sh").arg("-c").arg(&exec).spawn();
                    }
                }
            }
            Message::AppMapLoaded(map) => {
                self.app_map = map;
            }
            Message::ConfigChanged(config) => {
                self.config = config;
            }
            Message::LaunchPinned(idx) => {
                if let Some(app_id) = self.config.pinned_apps.get(idx).cloned() {
                    return self.update(Message::LaunchApp(app_id));
                }
            }
            Message::UnpinPinned(idx) => {
                if idx < self.config.pinned_apps.len() {
                    self.config.pinned_apps.remove(idx);
                    let _ = self.config.write_entry(&self.config_handler);
                }
            }
            Message::OpenSettings => {
                let _ = std::process::Command::new("cosmic-settings")
                    .arg("panel-applet")
                    .spawn();
            }
            Message::OpenAbout => {
                let _ = std::process::Command::new("cosmic-settings")
                    .arg("about")
                    .spawn();
            }
            Message::SurfaceAction(action) => {
                return Task::done(cosmic::Action::Cosmic(cosmic::app::Action::Surface(action)));
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let is_horizontal = self.core.applet.is_horizontal();
        let suggested_size = self.core.applet.suggested_size(false);
        let thickness = if is_horizontal { suggested_size.1 } else { suggested_size.0 } as f32;
        
        let icon_size_px = (thickness * 0.65).max(16.0);
        let font_size = (thickness * 0.40).max(11.0).min(14.0) as u16;

        let mut closed_pinned_apps = Vec::new();
        for (idx, app_id) in self.config.pinned_apps.iter().enumerate() {
            if !self.windows.iter().any(|(_, _, info)| &info.app_id == app_id) {
                closed_pinned_apps.push((idx, app_id.clone()));
            }
        }

        let filtered_windows: Vec<_> = if self.config.show_all_workspaces {
            self.windows.iter().collect()
        } else {
            self.windows.iter().filter(|(_, _, info)| {
                if info.workspace.is_empty() {
                    true
                } else {
                    info.workspace.iter().any(|w| self.active_workspaces.contains(w))
                }
            }).collect()
        };

        let list_data: (Element<'_, Self::Message>, f32) = if self.connection_finished {
            (
                widget::container(widget::text::text("Err"))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into(),
                40.0
            )
        } else if filtered_windows.is_empty() && closed_pinned_apps.is_empty() {
            let empty_content = widget::container(
                widget::icon::icon(widget::icon::from_name("view-list-symbolic").into())
                    .size(icon_size_px as u16)
                    .width(Length::Fixed(icon_size_px))
                    .height(Length::Fixed(icon_size_px))
            )
            .width(Length::Fixed(thickness))
            .height(Length::Fixed(thickness))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center);

            let key_binds = HashMap::new();
            let menu_items = vec![
                cosmic::widget::menu::Item::Button("About", None, AppAction::About),
                cosmic::widget::menu::Item::Button("Settings", None, AppAction::Settings),
            ];
            let menu_tree = cosmic::widget::menu::items(&key_binds, menu_items);

            (
                widget::context_menu(empty_content, Some(menu_tree))
                    .on_surface_action(Message::SurfaceAction)
                    .into(),
                thickness
            )
        } else if is_horizontal {
            let num_active = filtered_windows.len();
            let num_pinned = closed_pinned_apps.len();
            let num_windows = num_active + num_pinned;
            let b_width = self.core.applet.suggested_bounds.map(|b| b.width).unwrap_or(1000.0);
            
            let max_item_w = if b_width > 100.0 && num_active > 0 {
                let available = (b_width - (num_pinned as f32 * thickness)) / num_active as f32;
                if self.config.expand_centered {
                    available.max(40.0)
                } else {
                    available.min(160.0).max(40.0)
                }
            } else {
                160.0
            };

            let mut row = widget::row()
                .spacing(2)
                .align_y(Alignment::Center)
                .width(if self.config.expand_centered { Length::Fill } else { Length::Shrink })
                .height(Length::Fill);

            for (pin_idx, app_id) in closed_pinned_apps {
                let app_info = self.app_map.get(&app_id.to_lowercase()).cloned().unwrap_or_default();
                let icon_name = if app_info.icon.is_empty() { app_id.as_str() } else { app_info.icon.as_str() };
                
                let btn_content = widget::container(
                    widget::icon::icon(widget::icon::from_name(icon_name).into())
                        .size(icon_size_px as u16) 
                        .width(Length::Fixed(icon_size_px))
                        .height(Length::Fixed(icon_size_px))
                )
                .height(Length::Fixed(thickness))
                .width(Length::Fixed(thickness))
                .align_x(Alignment::Center)
                .align_y(Alignment::Center);

                let btn = widget::button::custom(btn_content)
                    .padding(0) 
                    .height(Length::Fixed(thickness))
                    .width(Length::Fixed(thickness))
                    .on_press(Message::LaunchApp(app_id.clone()))
                    .class(win11_button_style());

                let menu_items = vec![
                    cosmic::widget::menu::Item::Button("Launch".to_string(), None, WindowAction::LaunchPinned(pin_idx)),
                    cosmic::widget::menu::Item::Button("✓ Unpin from app tray".to_string(), None, WindowAction::UnpinPinned(pin_idx)),
                ];
                let key_binds = HashMap::new();
                let menu_tree = cosmic::widget::menu::items(&key_binds, menu_items);

                let btn_with_menu = widget::context_menu(btn, Some(menu_tree))
                    .on_surface_action(Message::SurfaceAction);

                row = row.push(btn_with_menu);
            }

            for (id, _handle, info) in filtered_windows {
                let is_focused = info.state.contains(&State::Activated);
                let app_info = get_app_info(&info.app_id, &info.title, &self.app_map);
                let title = if info.title.is_empty() { "Untitled".to_string() } else { info.title.clone() };

                let mut btn_content = widget::container(
                    widget::row()
                        .spacing(8)
                        .align_y(Alignment::Center)
                        .push(
                            widget::icon::icon(widget::icon::from_name(app_info.icon.as_str()).into())
                                .size(icon_size_px as u16) 
                                .width(Length::Fixed(icon_size_px))
                                .height(Length::Fixed(icon_size_px))
                        )
                        .push(
                            widget::text::text(title)
                                .size(font_size)
                                .width(Length::Fill)
                                .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
                        )
                )
                .height(Length::Fixed(thickness))
                .width(Length::Fixed(max_item_w)) // Strict fixed width for titles
                .padding([0, 14])
                .center_y(Length::Fill);

                if is_focused {
                    btn_content = btn_content.style(|theme: &cosmic::Theme| {
                        widget::container::Style {
                            border: Border {
                                color: Color::TRANSPARENT,
                                width: 0.0,
                                radius: 0.0.into(),
                            },
                            ..Default::default()
                        }
                    });

                    // Add a small indicator at the bottom
                    let indicator = widget::container(widget::column().width(Length::Fill).height(Length::Fixed(2.0)))
                        .width(Length::Fixed(max_item_w * 0.4))
                        .height(Length::Fixed(2.0))
                        .style(|theme: &cosmic::Theme| {
                            let cosmic = theme.cosmic();
                            widget::container::Style {
                                background: Some(Background::Color(Color::from(cosmic.accent_color()))),
                                border: Border {
                                    radius: 1.0.into(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        });


                    btn_content = widget::container(
                        widget::column()
                            .push(btn_content)
                            .push(
                                widget::row()
                                    .width(Length::Fill)
                                    .align_y(Alignment::End)
                                    .push(widget::horizontal_space())
                                    .push(indicator)
                                    .push(widget::horizontal_space())
                            )
                    )
                    .height(Length::Fixed(thickness))
                    .width(Length::Fixed(max_item_w))
                    .align_y(Alignment::End);
                }

                let btn = widget::button::custom(btn_content)
                    .padding(0) 
                    .height(Length::Fixed(thickness))
                    .width(Length::Fixed(max_item_w))
                    .on_press(Message::Activate(*id))
                    .class(win11_button_style());
                let mut menu_items = Vec::new();

                for (action_idx, action) in app_info.actions.iter().enumerate() {
                    menu_items.push(cosmic::widget::menu::Item::Button(
                        truncate_text(&action.name, self.config.context_menu_text_limit),
                        None,
                        WindowAction::Spawn(*id, action_idx),
                    ));
                }

                if !app_info.actions.is_empty() {
                    menu_items.push(cosmic::widget::menu::Item::Divider);
                }

                let same_app_windows: Vec<_> = self.windows.iter().filter(|(_, _, w_info)| w_info.app_id == info.app_id).collect();
                
                for (w_id, _, w_info) in &same_app_windows {
                    let w_title = if w_info.title.is_empty() { "Untitled".to_string() } else { w_info.title.clone() };
                    menu_items.push(cosmic::widget::menu::Item::Button(
                        truncate_text(&w_title, self.config.context_menu_text_limit),
                        None,
                        WindowAction::ActivateWindow(*w_id),
                    ));
                }
                
                if !same_app_windows.is_empty() {
                    menu_items.push(cosmic::widget::menu::Item::Divider);
                }

                let pin_text = if self.config.pinned_apps.contains(&info.app_id) {
                    "✓ Unpin from app tray".to_string()
                } else {
                    "Pin to app tray".to_string()
                };
                menu_items.push(cosmic::widget::menu::Item::Button(pin_text, None, WindowAction::Pin(*id)));
                menu_items.push(cosmic::widget::menu::Item::Divider);

                if same_app_windows.len() > 1 {
                    menu_items.push(cosmic::widget::menu::Item::Button("Quit All".to_string(), None, WindowAction::CloseAll(*id)));
                } else {
                    menu_items.push(cosmic::widget::menu::Item::Button("Quit".to_string(), None, WindowAction::Close(*id)));
                }

                let key_binds = HashMap::new();
                let menu_tree = cosmic::widget::menu::items(&key_binds, menu_items);

                let btn_with_menu = widget::context_menu(btn, Some(menu_tree))
                    .on_surface_action(Message::SurfaceAction);

                row = row.push(btn_with_menu);
            }

            let total_width = if self.config.expand_centered {
                self.core.applet.suggested_bounds.map(|b| b.width).unwrap_or(1000.0)
            } else {
                (num_pinned as f32 * thickness) 
                    + (num_active as f32 * max_item_w) 
                    + (num_windows.saturating_sub(1) as f32 * 2.0) // spacing is 2
            };
            
            (row.into(), total_width)
        } else {
            let num_active = filtered_windows.len();
            let num_pinned = closed_pinned_apps.len();
            let num_windows = num_active + num_pinned;
            
            let mut col = widget::column()
                .spacing(2)
                .align_x(Alignment::Center)
                .width(Length::Fixed(thickness))
                .height(Length::Shrink);

            for (pin_idx, app_id) in closed_pinned_apps {
                let app_info = self.app_map.get(&app_id.to_lowercase()).cloned().unwrap_or_default();
                let icon_name = if app_info.icon.is_empty() { app_id.as_str() } else { app_info.icon.as_str() };
                
                let btn = widget::button::custom(
                    widget::container(
                        widget::icon::icon(widget::icon::from_name(icon_name).into())
                            .size(icon_size_px as u16)
                            .width(Length::Fixed(icon_size_px))
                            .height(Length::Fixed(icon_size_px))
                    )
                    .width(Length::Fixed(thickness))
                    .height(Length::Fixed(thickness))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                )
                .padding(0)
                .width(Length::Fixed(thickness))
                .height(Length::Fixed(thickness))
                .on_press(Message::LaunchApp(app_id.clone()))
                .class(win11_button_style());

                let menu_items = vec![
                    cosmic::widget::menu::Item::Button("Launch".to_string(), None, WindowAction::LaunchPinned(pin_idx)),
                    cosmic::widget::menu::Item::Button("✓ Unpin from app tray".to_string(), None, WindowAction::UnpinPinned(pin_idx)),
                ];
                let key_binds = HashMap::new();
                let menu_tree = cosmic::widget::menu::items(&key_binds, menu_items);

                let btn_with_menu = widget::context_menu(btn, Some(menu_tree))
                    .on_surface_action(Message::SurfaceAction);

                col = col.push(btn_with_menu);
            }

            for (id, _handle, info) in filtered_windows {
                let is_focused = info.state.contains(&State::Activated);
                let app_info = get_app_info(&info.app_id, &info.title, &self.app_map);
                
                let mut btn_content = widget::container(
                    widget::icon::icon(widget::icon::from_name(app_info.icon.as_str()).into())
                        .size(icon_size_px as u16)
                        .width(Length::Fixed(icon_size_px))
                        .height(Length::Fixed(icon_size_px))
                )
                .width(Length::Fixed(thickness))
                .height(Length::Fixed(thickness))
                .align_x(Alignment::Center)
                .align_y(Alignment::Center);

                if is_focused {
                    let indicator = widget::container(widget::row().width(Length::Fixed(2.0)).height(Length::Fill))
                        .width(Length::Fixed(2.0))
                        .height(Length::Fixed(thickness * 0.4))
                        .style(|theme: &cosmic::Theme| {
                            let cosmic = theme.cosmic();
                            widget::container::Style {
                                background: Some(Background::Color(Color::from(cosmic.accent_color()))),
                                border: Border {
                                    radius: 1.0.into(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        });


                    btn_content = widget::container(
                        widget::row()
                            .push(indicator)
                            .push(btn_content)
                    )
                    .width(Length::Fixed(thickness))
                    .height(Length::Fixed(thickness))
                    .align_x(Alignment::Start);
                }

                let btn = widget::button::custom(btn_content)
                    .padding(0) 
                    .width(Length::Fixed(thickness))
                    .height(Length::Fixed(thickness))
                    .on_press(Message::Activate(*id))
                    .class(win11_button_style());

                let mut menu_items = Vec::new();

                for (action_idx, action) in app_info.actions.iter().enumerate() {
                    menu_items.push(cosmic::widget::menu::Item::Button(
                        truncate_text(&action.name, self.config.context_menu_text_limit),
                        None,
                        WindowAction::Spawn(*id, action_idx),
                    ));
                }

                if !app_info.actions.is_empty() {
                    menu_items.push(cosmic::widget::menu::Item::Divider);
                }

                let same_app_windows: Vec<_> = self.windows.iter().filter(|(_, _, w_info)| w_info.app_id == info.app_id).collect();
                
                for (w_id, _, w_info) in &same_app_windows {
                    let w_title = if w_info.title.is_empty() { "Untitled".to_string() } else { w_info.title.clone() };
                    menu_items.push(cosmic::widget::menu::Item::Button(
                        truncate_text(&w_title, self.config.context_menu_text_limit),
                        None,
                        WindowAction::ActivateWindow(*w_id),
                    ));
                }

                if !same_app_windows.is_empty() {
                    menu_items.push(cosmic::widget::menu::Item::Divider);
                }

                let pin_text = if self.config.pinned_apps.contains(&info.app_id) {
                    "✓ Unpin from app tray".to_string()
                } else {
                    "Pin to app tray".to_string()
                };
                menu_items.push(cosmic::widget::menu::Item::Button(pin_text, None, WindowAction::Pin(*id)));
                menu_items.push(cosmic::widget::menu::Item::Divider);

                if same_app_windows.len() > 1 {
                    menu_items.push(cosmic::widget::menu::Item::Button("Quit All".to_string(), None, WindowAction::CloseAll(*id)));
                } else {
                    menu_items.push(cosmic::widget::menu::Item::Button("Quit".to_string(), None, WindowAction::Close(*id)));
                }

                let key_binds = HashMap::new();
                let menu_tree = cosmic::widget::menu::items(&key_binds, menu_items);

                let btn_with_menu = widget::context_menu(btn, Some(menu_tree))
                    .on_surface_action(Message::SurfaceAction);

                col = col.push(btn_with_menu);
            }
            let total_height = (num_windows as f32 * thickness) + (num_windows.saturating_sub(1) as f32 * 2.0);
            (col.into(), total_height)
        };

        let (list, list_size) = list_data;
        let container = if is_horizontal {
            let container = widget::container(list);
            if self.config.expand_centered {
                container.center_x(Length::Fill)
                    .center_y(Length::Fixed(thickness))
                    .padding([0, 4])
            } else {
                container.width(Length::Fixed(list_size)).height(Length::Fixed(thickness))
            }
        } else {
            widget::container(list)
                .width(Length::Fixed(thickness))
                .height(Length::Fixed(list_size))
        };

        let mut limits = Limits::NONE;
        if is_horizontal {
            if self.config.expand_centered {
                let target_w = self.core.applet.suggested_bounds.map(|b| b.width).unwrap_or(4000.0);
                // By using list_size as min_width, we allow the panel to shrink the applet
                // if other applets need space.
                // By using target_w as max_width, we ensure it doesn't overlap neighbors.
                limits = limits.min_width(list_size.min(target_w)).max_width(target_w)
                               .min_height(thickness).max_height(thickness);
            } else {
                limits = limits.min_width(list_size).max_width(list_size)
                               .min_height(thickness).max_height(thickness);
            }
        } else {
            limits = limits.min_width(thickness).max_width(thickness)
                           .min_height(list_size).max_height(list_size);
        }

        self.core.applet.autosize_window(container)
            .limits(limits)
            .into()
    }

    fn view_window(&self, _id: window::Id) -> Element<'_, Self::Message> {
        widget::text::text("").into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::batch(vec![
            wayland_subscription().map(Message::WaylandUpdate),
            cosmic::cosmic_config::config_subscription::<&'static str, Config>(
                "window-list-config",
                Self::APP_ID.into(),
                1,
            )
            .map(|update| Message::ConfigChanged(update.config)),
        ])
    }

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
}

fn main() -> cosmic::iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    cosmic::applet::run::<WindowListApplet>(())
}
