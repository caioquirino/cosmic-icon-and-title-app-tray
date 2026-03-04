use cosmic_applet_window_list::wayland_subscription::{
    wayland_subscription, ToplevelUpdate, WaylandRequest, WaylandUpdate, ToplevelRequest,
};
use cctk::toplevel_info::ToplevelInfo;
use cctk::wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1;
use cosmic::app::{Core};
use cosmic::iced::{Alignment, Length, Subscription, Limits, window};
use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::widget::{self};
use cosmic::{Element, Task};
use tracing_subscriber::EnvFilter;
use tracing::{info, error};

struct WindowListApplet {
    core: Core,
    windows: Vec<(String, ToplevelInfo, ExtForeignToplevelHandleV1)>,
    wayland_tx: Option<cctk::sctk::reexports::calloop::channel::Sender<WaylandRequest>>,
    connection_finished: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    WaylandUpdate(WaylandUpdate),
    Activate(ExtForeignToplevelHandleV1),
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
        info!("Initializing WindowListApplet");
        (
            WindowListApplet {
                core,
                windows: Vec::new(),
                wayland_tx: None,
                connection_finished: false,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::WaylandUpdate(update) => match update {
                WaylandUpdate::Init(tx) => {
                    info!("Wayland subscription initialized");
                    self.wayland_tx = Some(tx);
                    self.connection_finished = false;
                }
                WaylandUpdate::Toplevel(toplevel_update) => match toplevel_update {
                    ToplevelUpdate::Add(info, handle) => {
                        info!("Adding window: {} ({})", info.title, info.app_id);
                        self.windows.push((info.identifier.clone(), info, handle));
                    }
                    ToplevelUpdate::Update(info, _handle) => {
                        if let Some(pos) = self.windows.iter().position(|(id, _, _)| id == &info.identifier) {
                            self.windows[pos].1 = info;
                        }
                    }
                    ToplevelUpdate::Remove(handle) => {
                        info!("Removing window handle");
                        self.windows.retain(|(_, _, h)| h != &handle);
                    }
                },
                WaylandUpdate::Finished => {
                    error!("Wayland connection finished unexpectedly");
                    self.connection_finished = true;
                    self.windows.clear();
                }
            },
            Message::Activate(handle) => {
                info!("Activating window");
                if let Some(tx) = self.wayland_tx.as_ref() {
                    let _ = tx.send(WaylandRequest::Toplevel(ToplevelRequest::Activate(handle)));
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let is_horizontal = self.core.applet.is_horizontal();
        let suggested_size = self.core.applet.suggested_size(false);
        let icon_size = 16;

        if self.connection_finished {
            return self.core.applet.autosize_window(
                widget::container(widget::text::text("Err"))
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
            )
            .into();
        }

        if self.windows.is_empty() {
            let panel_icon_size = suggested_size.0.max(20);
            return self.core.applet.autosize_window(
                widget::container(
                    widget::icon::icon(widget::icon::from_name("view-list-symbolic").into())
                        .size(panel_icon_size)
                )
                .width(Length::Fixed(panel_icon_size as f32))
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
            )
            .into();
        }

        let list: Element<'_, Self::Message> = if is_horizontal {
            let mut row = widget::row().spacing(4).align_y(Alignment::Center);
            for (_id, info, handle) in &self.windows {
                let app_id = if info.app_id.is_empty() { "sun-repository-symbolic".to_string() } else { info.app_id.clone() };
                let title = if info.title.is_empty() { "Untitled".to_string() } else { info.title.clone() };
                
                let btn_content = widget::row()
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .push(widget::icon::icon(widget::icon::from_name(&*app_id).into()).size(icon_size))
                    .push(
                        widget::text::text(title)
                            .size(13)
                            .width(Length::Fixed(120.0))
                            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
                    );

                row = row.push(
                    widget::button::custom(btn_content)
                        .padding([4, 12])
                        .on_press(Message::Activate(handle.clone()))
                        .class(cosmic::theme::Button::Standard)
                );
            }
            widget::scrollable(row).into()
        } else {
            let mut col = widget::column().spacing(4).align_x(Alignment::Center);
            for (_id, info, handle) in &self.windows {
                let app_id = if info.app_id.is_empty() { "sun-repository-symbolic".to_string() } else { info.app_id.clone() };
                col = col.push(
                    widget::button::custom(
                        widget::icon::icon(widget::icon::from_name(&*app_id).into()).size(icon_size)
                    )
                    .padding(4)
                    .on_press(Message::Activate(handle.clone()))
                    .class(cosmic::theme::Button::Standard)
                );
            }
            widget::scrollable(col).into()
        };

        let container = widget::container(list)
            .height(Length::Fill)
            .center_y(Length::Fill);

        let mut limits = Limits::NONE.min_width(1.0).min_height(1.0);
        if let Some(b) = self.core.applet.suggested_bounds {
            // Strictly respect the panel's width to avoid overlapping other segments
            limits = limits.max_width(if b.width > 1.0 { b.width } else { 2000.0 });
            // Cap height to panel height to avoid vertical bloat
            limits = limits.max_height(if b.height > 1.0 { b.height } else { 32.0 });
        } else {
            limits = limits.max_width(2000.0).max_height(32.0);
        }

        self.core.applet.autosize_window(container)
            .limits(limits)
            .into()
    }

    fn view_window(&self, _id: window::Id) -> Element<'_, Self::Message> {
        widget::text::text("").into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        wayland_subscription().map(Message::WaylandUpdate)
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
