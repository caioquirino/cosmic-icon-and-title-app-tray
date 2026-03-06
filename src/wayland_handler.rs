use crate::wayland_subscription::{ToplevelRequest, ToplevelUpdate, WaylandRequest, WaylandUpdate};
use cctk::{
    sctk::{
        self,
        registry::{ProvidesRegistryState, RegistryState},
        reexports::calloop_wayland_source::WaylandSource,
        reexports::calloop,
        seat::{SeatHandler, SeatState},
    },
    workspace::{WorkspaceHandler, WorkspaceState},
    toplevel_info::{ToplevelInfoHandler, ToplevelInfoState},
    toplevel_management::{ToplevelManagerHandler, ToplevelManagerState},
    wayland_client::{Connection, QueueHandle, WEnum, globals::{registry_queue_init, GlobalList}, protocol::wl_seat::WlSeat},
    wayland_protocols::ext::{
        foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
        workspace::v1::client::ext_workspace_handle_v1::{State as WorkspaceUpdateState, ExtWorkspaceHandleV1},
    },
};
use cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1;
use futures::channel::mpsc::UnboundedSender;
use std::os::unix::net::UnixStream;
use std::os::fd::{FromRawFd, RawFd};
use tracing::info;

struct AppData {
    exit: bool,
    tx: UnboundedSender<WaylandUpdate>,
    toplevel_info_state: ToplevelInfoState,
    toplevel_manager_state: ToplevelManagerState,
    seat_state: SeatState,
    workspace_state: WorkspaceState,
    registry_state: RegistryState,
}

impl ProvidesRegistryState for AppData {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    sctk::registry_handlers!();
}

impl SeatHandler for AppData {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
    fn new_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat, _: sctk::seat::Capability) {}
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat, _: sctk::seat::Capability) {}
}

impl WorkspaceHandler for AppData {
    fn workspace_state(&mut self) -> &mut WorkspaceState {
        &mut self.workspace_state
    }

    fn done(&mut self) {
        let active_workspaces = self
            .workspace_state
            .workspace_groups()
            .filter_map(|x| {
                x.workspaces
                    .iter()
                    .filter_map(|handle| self.workspace_state.workspace_info(handle))
                    .find(|w| w.state.contains(WorkspaceUpdateState::Active))
                    .map(|workspace| workspace.handle.clone())
            })
            .collect::<Vec<ExtWorkspaceHandleV1>>();
        let _ = self
            .tx
            .unbounded_send(WaylandUpdate::Workspace(active_workspaces));
    }
}

impl ToplevelManagerHandler for AppData {
    fn toplevel_manager_state(&mut self) -> &mut ToplevelManagerState {
        &mut self.toplevel_manager_state
    }
    fn capabilities(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: Vec<WEnum<zcosmic_toplevel_manager_v1::ZcosmicToplelevelManagementCapabilitiesV1>>,
    ) {}
}

impl ToplevelInfoHandler for AppData {
    fn toplevel_info_state(&mut self) -> &mut ToplevelInfoState {
        &mut self.toplevel_info_state
    }

    fn new_toplevel(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &ExtForeignToplevelHandleV1,
    ) {
        if let Some(info) = self.toplevel_info_state.info(toplevel) {
            let _ = self.tx.unbounded_send(WaylandUpdate::Toplevel(ToplevelUpdate::Add(info.clone(), toplevel.clone())));
        }
    }

    fn update_toplevel(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &ExtForeignToplevelHandleV1,
    ) {
        if let Some(info) = self.toplevel_info_state.info(toplevel) {
            let _ = self.tx.unbounded_send(WaylandUpdate::Toplevel(ToplevelUpdate::Update(info.clone(), toplevel.clone())));
        }
    }

    fn toplevel_closed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        toplevel: &ExtForeignToplevelHandleV1,
    ) {
        let _ = self.tx.unbounded_send(WaylandUpdate::Toplevel(ToplevelUpdate::Remove(toplevel.clone())));
    }
}

pub fn wayland_handler(
    tx: UnboundedSender<WaylandUpdate>,
    rx: calloop::channel::Channel<WaylandRequest>,
) {
    info!("Starting wayland_handler thread");

    let socket_var = std::env::var("X_PRIVILEGED_WAYLAND_SOCKET").ok();
    
    let mut connection_attempt: Option<(Connection, (GlobalList, _))> = None;

    if let Some(fd_str) = socket_var.as_ref() {
        if let Ok(fd) = fd_str.parse::<RawFd>() {
            if let Ok(_) = rustix::fs::fstat(unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) }) {
                let socket = unsafe { UnixStream::from_raw_fd(fd) };
                if let Ok(conn) = Connection::from_socket(socket) {
                    match registry_queue_init(&conn) {
                        Ok(res) => {
                            connection_attempt = Some((conn, res));
                        }
                        Err(_) => {}
                    }
                }
            }
        }
    }

    let (conn, (globals, event_queue)) = if let Some(res) = connection_attempt {
        res
    } else {
        let conn = Connection::connect_to_env().unwrap();
        let res = registry_queue_init(&conn).unwrap();
        (conn, res)
    };

    let mut event_loop = calloop::EventLoop::<AppData>::try_new().unwrap();
    let qh = event_queue.handle();
    let wayland_source = WaylandSource::new(conn.clone(), event_queue);
    let handle = event_loop.handle();
    wayland_source.insert(handle.clone()).unwrap();

    handle.insert_source(rx, |event, (), state| match event {
        calloop::channel::Event::Msg(req) => match req {
            WaylandRequest::Toplevel(req) => match req {
                ToplevelRequest::Activate(handle) => {
                   if let Some(info) = state.toplevel_info_state.info(&handle) {
                       if let Some(cosmic_toplevel) = &info.cosmic_toplevel {
                           if let Some(seat) = state.seat_state.seats().next() {
                               let manager = &state.toplevel_manager_state.manager;
                               manager.activate(cosmic_toplevel, &seat);
                           }
                       }
                   }
                }
                ToplevelRequest::Quit(handle) => {
                   if let Some(info) = state.toplevel_info_state.info(&handle) {
                       if let Some(cosmic_toplevel) = &info.cosmic_toplevel {
                           state.toplevel_manager_state.manager.close(cosmic_toplevel);
                       }
                   }
                }
            }
        },
        calloop::channel::Event::Closed => {
            state.exit = true;
        }
    }).unwrap();

    let registry_state = RegistryState::new(&globals);

    let mut app_data = AppData {
        exit: false,
        tx,
        toplevel_info_state: ToplevelInfoState::new(&registry_state, &qh),
        toplevel_manager_state: ToplevelManagerState::new(&registry_state, &qh),
        seat_state: SeatState::new(&globals, &qh),
        workspace_state: WorkspaceState::new(&registry_state, &qh),
        registry_state,
    };

    loop {
        if app_data.exit { break; }
        if let Err(_) = event_loop.dispatch(None, &mut app_data) {
            break;
        }
    }
    info!("wayland_handler thread exiting");
}

sctk::delegate_registry!(AppData);
sctk::delegate_seat!(AppData);
cctk::delegate_workspace!(AppData);
cctk::delegate_toplevel_info!(AppData);
cctk::delegate_toplevel_manager!(AppData);
