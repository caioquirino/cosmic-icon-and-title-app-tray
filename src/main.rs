use cosmic_applet_window_list::{
    app_map::{AppInfo, build_app_map, get_app_info},
    config::Config,
    styles::{strip_exec_args, truncate_text, win11_button_style},
    wayland_subscription::{wayland_subscription, ToplevelRequest, ToplevelUpdate, WaylandRequest, WaylandUpdate},
};
use cctk::toplevel_info::ToplevelInfo;
use cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_handle_v1::State;
use cctk::wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1;
use cctk::wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1::ExtWorkspaceHandleV1;
use cosmic::app::Core;
use cosmic::iced::{Alignment, Background, Border, Color, Length, Limits, Subscription, window};
use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::widget::{self};
use cosmic::{Element, Task};
use cosmic::cosmic_config::CosmicConfigEntry;
use tracing_subscriber::EnvFilter;
use std::collections::HashMap;

// ── Window entry ──────────────────────────────────────────────────────────────

struct WindowEntry {
    id: usize,
    handle: ExtForeignToplevelHandleV1,
    info: ToplevelInfo,
}

// ── App state ─────────────────────────────────────────────────────────────────

struct WindowListApplet {
    core: Core,
    windows: Vec<WindowEntry>,
    next_id: usize,
    active_workspaces: Vec<ExtWorkspaceHandleV1>,
    wayland_tx: Option<cctk::sctk::reexports::calloop::channel::Sender<WaylandRequest>>,
    connection_finished: bool,
    app_map: HashMap<String, AppInfo>,
    config: Config,
    config_handler: Option<cosmic::cosmic_config::Config>,
}

// ── Messages ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    WaylandUpdate(WaylandUpdate),
    Activate(usize),
    Close(usize),
    CloseAll(usize),
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

// ── Menu actions ──────────────────────────────────────────────────────────────

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

// ── Free functions (no app state needed) ──────────────────────────────────────

fn pinned_menu_items(pin_idx: usize) -> Vec<cosmic::widget::menu::Item<WindowAction, String>> {
    vec![
        cosmic::widget::menu::Item::Button("Launch".to_string(), None, WindowAction::LaunchPinned(pin_idx)),
        cosmic::widget::menu::Item::Button("✓ Unpin from app tray".to_string(), None, WindowAction::UnpinPinned(pin_idx)),
    ]
}

fn active_indicator_h<'a>(width: f32) -> cosmic::Element<'a, Message> {
    widget::container(widget::column().width(Length::Fill).height(Length::Fixed(2.0)))
        .width(Length::Fixed(width))
        .height(Length::Fixed(2.0))
        .style(|theme: &cosmic::Theme| {
            let cosmic = theme.cosmic();
            widget::container::Style {
                background: Some(Background::Color(Color::from(cosmic.accent_color()))),
                border: Border { radius: 1.0.into(), ..Default::default() },
                ..Default::default()
            }
        })
        .into()
}

fn active_indicator_v<'a>(height: f32) -> cosmic::Element<'a, Message> {
    widget::container(widget::row().width(Length::Fixed(2.0)).height(Length::Fill))
        .width(Length::Fixed(2.0))
        .height(Length::Fixed(height))
        .style(|theme: &cosmic::Theme| {
            let cosmic = theme.cosmic();
            widget::container::Style {
                background: Some(Background::Color(Color::from(cosmic.accent_color()))),
                border: Border { radius: 1.0.into(), ..Default::default() },
                ..Default::default()
            }
        })
        .into()
}

// ── View helpers (need app state) ─────────────────────────────────────────────

impl WindowListApplet {
    fn window_menu_items(
        &self,
        id: usize,
        info: &ToplevelInfo,
        app_info: &AppInfo,
    ) -> Vec<cosmic::widget::menu::Item<WindowAction, String>> {
        let mut items = Vec::new();

        for (action_idx, action) in app_info.actions.iter().enumerate() {
            items.push(cosmic::widget::menu::Item::Button(
                truncate_text(&action.name, self.config.context_menu_text_limit),
                None,
                WindowAction::Spawn(id, action_idx),
            ));
        }
        if !app_info.actions.is_empty() {
            items.push(cosmic::widget::menu::Item::Divider);
        }

        let same_app_windows: Vec<&WindowEntry> = self.windows.iter()
            .filter(|w| w.info.app_id == info.app_id)
            .collect();
        for w in &same_app_windows {
            let w_title = if w.info.title.is_empty() { "Untitled".to_string() } else { w.info.title.clone() };
            items.push(cosmic::widget::menu::Item::Button(
                truncate_text(&w_title, self.config.context_menu_text_limit),
                None,
                WindowAction::ActivateWindow(w.id),
            ));
        }
        if !same_app_windows.is_empty() {
            items.push(cosmic::widget::menu::Item::Divider);
        }

        let pin_text = if self.config.pinned_apps.contains(&info.app_id) {
            "✓ Unpin from app tray".to_string()
        } else {
            "Pin to app tray".to_string()
        };
        items.push(cosmic::widget::menu::Item::Button(pin_text, None, WindowAction::Pin(id)));
        items.push(cosmic::widget::menu::Item::Divider);

        if same_app_windows.len() > 1 {
            items.push(cosmic::widget::menu::Item::Button("Quit All".to_string(), None, WindowAction::CloseAll(id)));
        } else {
            items.push(cosmic::widget::menu::Item::Button("Quit".to_string(), None, WindowAction::Close(id)));
        }

        items
    }

    fn view_pinned_item<'a>(
        &'a self,
        pin_idx: usize,
        app_id: &str,
        thickness: f32,
        icon_size_px: f32,
    ) -> cosmic::Element<'a, Message> {
        let app_info = self.app_map.get(&app_id.to_lowercase()).cloned().unwrap_or_default();
        let icon_name = if app_info.icon.is_empty() { app_id.to_string() } else { app_info.icon };

        let btn_content = widget::container(
            widget::icon::from_name(icon_name.as_str())
                .size(icon_size_px as u16)
                .prefer_svg(true)
                .icon()
                .width(Length::Fixed(icon_size_px))
                .height(Length::Fixed(icon_size_px)),
        )
        .width(Length::Fixed(thickness))
        .height(Length::Fixed(thickness))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

        let btn = widget::button::custom(btn_content)
            .padding(0)
            .width(Length::Fixed(thickness))
            .height(Length::Fixed(thickness))
            .on_press(Message::LaunchApp(app_id.to_string()))
            .class(win11_button_style());

        let menu_tree = cosmic::widget::menu::items(&HashMap::new(), pinned_menu_items(pin_idx));
        widget::context_menu(btn, Some(menu_tree))
            .on_surface_action(Message::SurfaceAction)
            .into()
    }

    fn view_window_item_horizontal<'a>(
        &'a self,
        id: usize,
        info: &ToplevelInfo,
        active_item_width: f32,
        thickness: f32,
        icon_size_px: f32,
        font_size: u16,
    ) -> cosmic::Element<'a, Message> {
        let is_focused = info.state.contains(&State::Activated);
        let app_info = get_app_info(&info.app_id, &self.app_map);
        let title = if info.title.is_empty() { "Untitled".to_string() } else { info.title.clone() };

        let mut content = widget::container(
            widget::row()
                .spacing(8)
                .align_y(Alignment::Center)
                .push(
                    widget::icon::from_name(app_info.icon.as_str())
                        .size(icon_size_px as u16)
                        .prefer_svg(true)
                        .icon()
                        .width(Length::Fixed(icon_size_px))
                        .height(Length::Fixed(icon_size_px)),
                )
                .push(
                    widget::text::text(title)
                        .size(font_size)
                        .width(Length::Fill)
                        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1))),
                ),
        )
        .height(Length::Fixed(thickness))
        .width(Length::Fixed(active_item_width))
        .padding([0, 14])
        .center_y(Length::Fill);

        if is_focused {
            content = content.style(|_theme: &cosmic::Theme| widget::container::Style {
                border: Border { color: Color::TRANSPARENT, width: 0.0, radius: 0.0.into() },
                ..Default::default()
            });
            content = widget::container(
                widget::column()
                    .push(content)
                    .push(
                        widget::row()
                            .width(Length::Fill)
                            .align_y(Alignment::End)
                            .push(widget::horizontal_space())
                            .push(active_indicator_h(active_item_width * 0.4))
                            .push(widget::horizontal_space()),
                    ),
            )
            .height(Length::Fixed(thickness))
            .width(Length::Fixed(active_item_width))
            .align_y(Alignment::End);
        }

        let btn = widget::button::custom(content)
            .padding(0)
            .height(Length::Fixed(thickness))
            .width(Length::Fixed(active_item_width))
            .on_press(Message::Activate(id))
            .class(win11_button_style());

        let menu_tree = cosmic::widget::menu::items(&HashMap::new(), self.window_menu_items(id, info, &app_info));
        widget::context_menu(btn, Some(menu_tree))
            .on_surface_action(Message::SurfaceAction)
            .into()
    }

    fn view_window_item_vertical<'a>(
        &'a self,
        id: usize,
        info: &ToplevelInfo,
        thickness: f32,
        icon_size_px: f32,
    ) -> cosmic::Element<'a, Message> {
        let is_focused = info.state.contains(&State::Activated);
        let app_info = get_app_info(&info.app_id, &self.app_map);

        let mut content = widget::container(
            widget::icon::from_name(app_info.icon.as_str())
                .size(icon_size_px as u16)
                .prefer_svg(true)
                .icon()
                .width(Length::Fixed(icon_size_px))
                .height(Length::Fixed(icon_size_px)),
        )
        .width(Length::Fixed(thickness))
        .height(Length::Fixed(thickness))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

        if is_focused {
            content = widget::container(
                widget::row()
                    .push(active_indicator_v(thickness * 0.4))
                    .push(content),
            )
            .width(Length::Fixed(thickness))
            .height(Length::Fixed(thickness))
            .align_x(Alignment::Start);
        }

        let btn = widget::button::custom(content)
            .padding(0)
            .width(Length::Fixed(thickness))
            .height(Length::Fixed(thickness))
            .on_press(Message::Activate(id))
            .class(win11_button_style());

        let menu_tree = cosmic::widget::menu::items(&HashMap::new(), self.window_menu_items(id, info, &app_info));
        widget::context_menu(btn, Some(menu_tree))
            .on_surface_action(Message::SurfaceAction)
            .into()
    }
}

// ── cosmic::Application impl ──────────────────────────────────────────────────

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
        let config_handler = cosmic::cosmic_config::Config::new(Self::APP_ID, 1).ok();
        let config = config_handler.as_ref()
            .and_then(|h| Config::get_entry(h).ok())
            .unwrap_or_default();
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
            Task::perform(
                tokio::task::spawn_blocking(build_app_map),
                |result| cosmic::Action::App(Message::AppMapLoaded(result.unwrap())),
            ),
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
                        if !self.windows.iter().any(|w| w.handle == handle) {
                            self.windows.push(WindowEntry { id: self.next_id, handle, info });
                            self.next_id += 1;
                        }
                    }
                    ToplevelUpdate::Update(info, handle) => {
                        if let Some(pos) = self.windows.iter().position(|w| w.handle == handle) {
                            self.windows[pos].info = info;
                        } else {
                            self.windows.push(WindowEntry { id: self.next_id, handle, info });
                            self.next_id += 1;
                        }
                    }
                    ToplevelUpdate::Remove(handle) => {
                        self.windows.retain(|w| w.handle != handle);
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
            Message::Activate(id) => {
                if let Some(w) = self.windows.iter().find(|w| w.id == id) {
                    if let Some(tx) = self.wayland_tx.as_ref() {
                        let _ = tx.send(WaylandRequest::Toplevel(ToplevelRequest::Activate(w.handle.clone())));
                    }
                }
            }
            Message::Close(id) => {
                if let Some(w) = self.windows.iter().find(|w| w.id == id) {
                    if let Some(tx) = self.wayland_tx.as_ref() {
                        let _ = tx.send(WaylandRequest::Toplevel(ToplevelRequest::Quit(w.handle.clone())));
                    }
                }
            }
            Message::CloseAll(id) => {
                if let Some(app_id) = self.windows.iter().find(|w| w.id == id).map(|w| w.info.app_id.clone()) {
                    let handles: Vec<_> = self.windows.iter()
                        .filter(|w| w.info.app_id == app_id)
                        .map(|w| w.handle.clone())
                        .collect();
                    if let Some(tx) = self.wayland_tx.as_ref() {
                        for handle in handles {
                            let _ = tx.send(WaylandRequest::Toplevel(ToplevelRequest::Quit(handle)));
                        }
                    }
                }
            }
            Message::Pin(id) => {
                if let Some(app_id) = self.windows.iter().find(|w| w.id == id).map(|w| w.info.app_id.clone()) {
                    if self.config.pinned_apps.contains(&app_id) {
                        self.config.pinned_apps.retain(|a| a != &app_id);
                    } else {
                        self.config.pinned_apps.push(app_id);
                    }
                    if let Some(handler) = &self.config_handler {
                        let _ = self.config.write_entry(handler);
                    }
                }
            }
            Message::LaunchApp(app_id) => {
                if let Some(info) = self.app_map.get(&app_id.to_lowercase()) {
                    if let Some(ref exec) = info.main_exec {
                        let _ = std::process::Command::new("sh").arg("-c").arg(strip_exec_args(exec)).spawn();
                    }
                }
            }
            Message::Spawn(window_id, action_idx) => {
                if let Some(info) = self.windows.iter().find(|w| w.id == window_id).map(|w| &w.info) {
                    let app_info = get_app_info(&info.app_id, &self.app_map);
                    if let Some(action) = app_info.actions.get(action_idx) {
                        let _ = std::process::Command::new("sh").arg("-c").arg(strip_exec_args(&action.exec)).spawn();
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
                    if let Some(handler) = &self.config_handler {
                        let _ = self.config.write_entry(handler);
                    }
                }
            }
            Message::OpenSettings => {
                let _ = std::process::Command::new("cosmic-settings").arg("panel-applet").spawn();
            }
            Message::OpenAbout => {
                let _ = std::process::Command::new("cosmic-settings").arg("about").spawn();
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

        let closed_pinned: Vec<(usize, String)> = self.config.pinned_apps.iter().enumerate()
            .filter(|(_, app_id)| !self.windows.iter().any(|w| &w.info.app_id == *app_id))
            .map(|(idx, app_id)| (idx, app_id.clone()))
            .collect();

        let filtered_windows: Vec<&WindowEntry> = if self.config.show_all_workspaces {
            self.windows.iter().collect()
        } else {
            self.windows.iter().filter(|entry| {
                entry.info.workspace.is_empty()
                    || entry.info.workspace.iter().any(|w| self.active_workspaces.contains(w))
            }).collect()
        };

        let (list, list_size) = if self.connection_finished {
            let err = widget::container(widget::text::text("Err"))
                .center_x(Length::Fill)
                .center_y(Length::Fill);
            (err.into(), 40.0f32)
        } else if filtered_windows.is_empty() && closed_pinned.is_empty() {
            let placeholder = widget::container(
                widget::icon::from_name("view-list-symbolic")
                    .size(icon_size_px as u16)
                    .prefer_svg(true)
                    .icon()
                    .width(Length::Fixed(icon_size_px))
                    .height(Length::Fixed(icon_size_px)),
            )
            .width(Length::Fixed(thickness))
            .height(Length::Fixed(thickness))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center);

            let menu_tree = cosmic::widget::menu::items(&HashMap::new(), vec![
                cosmic::widget::menu::Item::Button("About", None, AppAction::About),
                cosmic::widget::menu::Item::Button("Settings", None, AppAction::Settings),
            ]);
            let element: cosmic::Element<'_, Message> = widget::context_menu(placeholder, Some(menu_tree))
                .on_surface_action(Message::SurfaceAction)
                .into();
            (element, thickness)
        } else if is_horizontal {
            let num_active = filtered_windows.len();
            let num_pinned = closed_pinned.len();
            let b_width = self.core.applet.suggested_bounds.map(|b| b.width).unwrap_or(1000.0);

            let active_item_width = if b_width > 10.0 && num_active > 0 {
                let spacing_total = (num_active + num_pinned).saturating_sub(1) as f32 * 2.0;
                let pinned_total = num_pinned as f32 * thickness;
                let available = (b_width - spacing_total - pinned_total) / num_active as f32;
                if self.config.expand_centered {
                    available.min(self.config.item_max_width).max(thickness)
                } else {
                    thickness.min(160.0)
                }
            } else {
                thickness
            };

            let mut row = widget::row()
                .spacing(2)
                .align_y(Alignment::Center)
                .width(if self.config.expand_centered { Length::Fill } else { Length::Shrink })
                .height(Length::Fill);

            for (pin_idx, app_id) in &closed_pinned {
                row = row.push(self.view_pinned_item(*pin_idx, app_id, thickness, icon_size_px));
            }
            for entry in &filtered_windows {
                row = row.push(self.view_window_item_horizontal(entry.id, &entry.info, active_item_width, thickness, icon_size_px, font_size));
            }

            let total_width = if self.config.expand_centered {
                self.core.applet.suggested_bounds.map(|b| b.width).unwrap_or(1000.0)
            } else {
                (num_pinned as f32 * thickness)
                    + (num_active as f32 * active_item_width)
                    + ((num_active + num_pinned).saturating_sub(1) as f32 * 2.0)
            };
            (row.into(), total_width)
        } else {
            let num_windows = filtered_windows.len() + closed_pinned.len();

            let mut col = widget::column()
                .spacing(2)
                .align_x(Alignment::Center)
                .width(Length::Fixed(thickness))
                .height(Length::Shrink);

            for (pin_idx, app_id) in &closed_pinned {
                col = col.push(self.view_pinned_item(*pin_idx, app_id, thickness, icon_size_px));
            }
            for entry in &filtered_windows {
                col = col.push(self.view_window_item_vertical(entry.id, &entry.info, thickness, icon_size_px));
            }

            let total_height = (num_windows as f32 * thickness) + (num_windows.saturating_sub(1) as f32 * 2.0);
            (col.into(), total_height)
        };

        let container = if is_horizontal {
            let container = widget::container(list);
            if self.config.expand_centered {
                container.center_x(Length::Fill).center_y(Length::Fixed(thickness)).padding([0, 4])
            } else {
                container.width(Length::Fixed(list_size)).height(Length::Fixed(thickness))
            }
        } else {
            widget::container(list).width(Length::Fixed(thickness)).height(Length::Fixed(list_size))
        };

        let mut limits = Limits::NONE;
        if is_horizontal {
            if self.config.expand_centered {
                let target_w = self.core.applet.suggested_bounds.map(|b| b.width).unwrap_or(4000.0);
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

        self.core.applet.autosize_window(container).limits(limits).into()
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
