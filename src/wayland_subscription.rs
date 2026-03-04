use cctk::{
    sctk::reexports::calloop,
    wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    wayland_protocols::ext::workspace::v1::client::ext_workspace_handle_v1::ExtWorkspaceHandleV1,
    toplevel_info::ToplevelInfo,
};
use cosmic::iced::{Subscription, stream};
use futures::{
    StreamExt, SinkExt,
    channel::mpsc::unbounded,
};

use crate::wayland_handler::wayland_handler;

#[derive(Clone, Debug)]
pub enum WaylandUpdate {
    Init(calloop::channel::Sender<WaylandRequest>),
    Finished,
    Toplevel(ToplevelUpdate),
    Workspace(Vec<ExtWorkspaceHandleV1>),
}

#[derive(Clone, Debug)]
pub enum ToplevelUpdate {
    Add(ToplevelInfo, ExtForeignToplevelHandleV1),
    Update(ToplevelInfo, ExtForeignToplevelHandleV1),
    Remove(ExtForeignToplevelHandleV1),
}

#[derive(Clone, Debug)]
pub enum WaylandRequest {
    Toplevel(ToplevelRequest),
}

#[derive(Debug, Clone)]
pub enum ToplevelRequest {
    Activate(ExtForeignToplevelHandleV1),
    Minimize(ExtForeignToplevelHandleV1),
    Quit(ExtForeignToplevelHandleV1),
}

pub fn wayland_subscription() -> Subscription<WaylandUpdate> {
    struct WaylandWorker;

    Subscription::run_with_id(
        std::any::TypeId::of::<WaylandWorker>(),
        stream::channel(50, move |mut output| async move {
            let (calloop_tx, calloop_rx) = calloop::channel::channel();
            let (toplevel_tx, mut toplevel_rx) = unbounded();
            
            std::thread::spawn(move || {
                wayland_handler(toplevel_tx, calloop_rx);
            });

            let _ = output.send(WaylandUpdate::Init(calloop_tx)).await;

            while let Some(update) = toplevel_rx.next().await {
                let _ = output.send(update).await;
            }
            
            let _ = output.send(WaylandUpdate::Finished).await;
            
            futures::future::pending().await
        }),
    )
}
