mod config;

use cosmic_applet_window_list::wayland_subscription::{
    wayland_subscription, ToplevelUpdate, WaylandRequest, WaylandUpdate, ToplevelRequest,
};
use cctk::toplevel_info::ToplevelInfo;
use cctk::wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1;
use cctk::wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1::ExtWorkspaceHandleV1;
use cosmic::app::{Core};
use cosmic::iced::{Alignment, Length, Subscription, Limits, window, Background, Color};
use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::widget::{self};
use cosmic::{Element, Task};
use tracing_subscriber::EnvFilter;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::config::Config;
use cosmic::cosmic_config::CosmicConfigEntry;

struct WindowListApplet {
    core: Core,
    windows: Vec<(ExtForeignToplevelHandleV1, ToplevelInfo)>,
    active_workspaces: Vec<ExtWorkspaceHandleV1>,
    wayland_tx: Option<cctk::sctk::reexports::calloop::channel::Sender<WaylandRequest>>,
    connection_finished: bool,
    icon_map: HashMap<String, String>,
    config: Config,
}

#[derive(Debug, Clone)]
pub enum Message {
    WaylandUpdate(WaylandUpdate),
    Activate(ExtForeignToplevelHandleV1),
    IconMapLoaded(HashMap<String, String>),
    ConfigChanged(Config),
}

fn build_icon_map() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut paths = vec![
        PathBuf::from("/usr/share/applications"),
    ];
    
    if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(home).join(".local/share/applications"));
    }

    for path in paths {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().map_or(false, |e| e == "desktop") {
                    if let Ok(content) = fs::read_to_string(&p) {
                        let mut icon = None;
                        let mut wm_class = None;
                        for line in content.lines() {
                            if line.starts_with("Icon=") {
                                icon = Some(line[5..].trim().to_string());
                            } else if line.starts_with("StartupWMClass=") {
                                wm_class = Some(line[15..].trim().to_string());
                            }
                        }
                        
                        let filename = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
                        
                        if let Some(i) = icon {
                            if let Some(w) = wm_class {
                                map.insert(w.to_lowercase(), i.clone());
                            }
                            map.insert(filename.to_lowercase(), i);
                        }
                    }
                }
            }
        }
    }
    map
}

fn get_icon_name(app_id: &str, title: &str, map: &HashMap<String, String>) -> String {
    let lower_app_id = app_id.to_lowercase();
    let lower_title = title.to_lowercase();

    if app_id.is_empty() {
        if lower_title.contains("intellij") || lower_title.contains("idea") { return "intellij-idea".to_string(); }
        if lower_title.contains("zen") { return "zen-browser".to_string(); }
        if lower_title.contains("settings") { return "com.system76.CosmicSettings".to_string(); }
        return "application-x-executable-symbolic".to_string();
    }

    if let Some(mapped) = map.get(&lower_app_id) {
        return mapped.clone();
    }

    if lower_app_id.contains("cosmic-settings") || lower_title.contains("cosmic settings") {
        return "com.system76.CosmicSettings".to_string();
    }
    if lower_app_id.contains("idea") || lower_app_id.contains("intellij") || lower_title.contains("intellij") { 
        return "intellij-idea".to_string(); 
    }
    if lower_app_id == "zen" || lower_app_id.contains("zen-browser") || lower_title.contains("zen browser") { 
        return "zen-browser".to_string(); 
    }
    if lower_app_id == "alacritty" { return "alacritty".to_string(); }
    
    if app_id.starts_with('/') {
        if let Some(name) = app_id.split('/').last() {
            return name.to_lowercase();
        }
    }

    if lower_app_id.contains('.') {
        if let Some(last) = lower_app_id.split('.').last() {
            if last.len() > 3 { return last.to_string(); }
        }
    }

    lower_app_id
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
        let config = cosmic::cosmic_config::Config::new(Self::APP_ID, 1)
            .ok()
            .and_then(|c| Config::get_entry(&c).ok())
            .unwrap_or_default();
        (
            WindowListApplet {
                core,
                windows: Vec::new(),
                active_workspaces: Vec::new(),
                wayland_tx: None,
                connection_finished: false,
                icon_map: HashMap::new(),
                config,
            },
            Task::perform(async { build_icon_map() }, |m| cosmic::Action::App(Message::IconMapLoaded(m))),
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
                        if !self.windows.iter().any(|(h, _)| h == &handle) {
                            self.windows.push((handle, info));
                        }
                    }
                    ToplevelUpdate::Update(info, handle) => {
                        if let Some(pos) = self.windows.iter().position(|(h, _)| h == &handle) {
                            self.windows[pos].1 = info;
                        } else {
                            self.windows.push((handle, info));
                        }
                    }
                    ToplevelUpdate::Remove(handle) => {
                        self.windows.retain(|(h, _)| h != &handle);
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
            Message::Activate(handle) => {
                if let Some(tx) = self.wayland_tx.as_ref() {
                    let _ = tx.send(WaylandRequest::Toplevel(ToplevelRequest::Activate(handle)));
                }
            }
            Message::IconMapLoaded(map) => {
                self.icon_map = map;
            }
            Message::ConfigChanged(config) => {
                self.config = config;
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let is_horizontal = self.core.applet.is_horizontal();
        let height = self.core.applet.suggested_size(false).1 as f32;
        let suggested_size = self.core.applet.suggested_size(false);
        
        let icon_size_px = (height * 0.65).max(16.0);
        let font_size = (height * 0.40).max(11.0).min(14.0) as u16;

        let filtered_windows: Vec<_> = if self.config.show_all_workspaces {
            self.windows.iter().collect()
        } else {
            self.windows.iter().filter(|(_, info)| {
                if info.workspace.is_empty() {
                    true
                } else {
                    info.workspace.iter().any(|w| self.active_workspaces.contains(w))
                }
            }).collect()
        };

        let list: Element<'_, Self::Message> = if self.connection_finished {
            widget::container(widget::text::text("Err"))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        } else if filtered_windows.is_empty() {
            let panel_icon_size = suggested_size.0.max(20);
            widget::container(
                widget::icon::icon(widget::icon::from_name("view-list-symbolic").into())
                    .size(64)
                    .width(Length::Fixed(panel_icon_size as f32))
                    .height(Length::Fixed(panel_icon_size as f32))
            )
            .width(Length::Fixed(panel_icon_size as f32))
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        } else if is_horizontal {
            let num_windows = filtered_windows.len();
            let b_width = self.core.applet.suggested_bounds.map(|b| b.width).unwrap_or(1000.0);
            let max_item_w = if b_width > 100.0 {
                (b_width / num_windows as f32).min(160.0).max(40.0)
            } else {
                160.0
            };

            let mut row = widget::row()
                .spacing(2)
                .align_y(Alignment::Center)
                .height(Length::Fill);

            for (handle, info) in filtered_windows {
                let icon_name = get_icon_name(&info.app_id, &info.title, &self.icon_map);
                let title = if info.title.is_empty() { "Untitled".to_string() } else { info.title.clone() };
                
                let btn_content = widget::container(
                    widget::row()
                        .spacing(8)
                        .align_y(Alignment::Center)
                        .push(
                            widget::icon::icon(widget::icon::from_name(&*icon_name).into())
                                .size(256) 
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
                .height(Length::Fill)
                .width(Length::Fixed(max_item_w - 24.0))
                .center_y(Length::Fill);

                row = row.push(
                    widget::button::custom(btn_content)
                        .padding([0, 14]) 
                        .height(Length::Fill)
                        .on_press(Message::Activate(handle.clone()))
                        .class(win11_button_style())
                );
            }
            row.into()
        } else {
            let mut col = widget::column()
                .spacing(2)
                .align_x(Alignment::Center)
                .width(Length::Fill);

            for (handle, info) in filtered_windows {
                let icon_name = get_icon_name(&info.app_id, &info.title, &self.icon_map);
                col = col.push(
                    widget::button::custom(
                        widget::container(
                            widget::icon::icon(widget::icon::from_name(&*icon_name).into())
                                .size(256)
                                .width(Length::Fixed(icon_size_px))
                                .height(Length::Fixed(icon_size_px))
                        )
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill)
                    )
                    .padding(0)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .on_press(Message::Activate(handle.clone()))
                    .class(win11_button_style())
                );
            }
            col.into()
        };

        let container = widget::container(list)
            .height(Length::Fixed(height))
            .width(Length::Shrink)
            .center_y(Length::Fill)
            .center_x(Length::Fill);

        let mut limits = Limits::NONE.min_width(1.0).min_height(1.0);
        if let Some(b) = self.core.applet.suggested_bounds {
            limits = limits.max_width(if b.width > 40.0 { b.width } else { 1600.0 });
            limits = limits.max_height(height);
        } else {
            limits = limits.max_width(1600.0).max_height(height);
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
