//! File drops on Wayland. winit 0.30 delivers `DroppedFile` on X11,
//! Windows and macOS and nothing on Wayland, so this module attaches a
//! second event queue to the display winit opened, binds a data device
//! on every seat, accepts `text/uri-list` drops over the window's own
//! surface and hands the paths to the app through its waker. The queue
//! runs on its own thread and shares the display's socket the way
//! libwayland is built for. The thread must end before the display is
//! freed: it waits on the display's descriptor and on a stop pipe, and
//! the app drops its handle, which closes the pipe and waits for the
//! thread, before anything of the window is dropped. On the stop the
//! thread's objects then drop on a live display; on a failure of the
//! display they are forgotten, as the process is ending.

use std::collections::HashMap;
use std::ffi::c_void;
use std::io::{ErrorKind, Read};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use rustix::event::{poll, PollFd, PollFlags};
use wayland_backend::client::{Backend, ObjectId, WaylandError};
use wayland_client::protocol::wl_data_device::{self, WlDataDevice};
use wayland_client::protocol::wl_data_device_manager::{DndAction, WlDataDeviceManager};
use wayland_client::protocol::wl_data_offer::{self, WlDataOffer};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{event_created_child, Connection, Dispatch, EventQueue, Proxy, QueueHandle};

use crate::doc::images::Waker;

const URI_LIST: &str = "text/uri-list";

/// The drop thread's handle: the paths dropped since the last take,
/// and the thread itself, which ends when the handle is dropped.
pub struct Drops {
    dropped: Dropped,
    /// The app's end of the stop pipe; closing it wakes the thread.
    stop: Option<UnixStream>,
    thread: Option<JoinHandle<()>>,
}

impl Drops {
    /// The dropped paths in drop order, emptied.
    pub fn take(&self) -> Vec<PathBuf> {
        self.dropped.take()
    }
}

/// The handle is dropped before the window: the thread ends while the
/// display is alive, so its queue and its objects drop on a live
/// display. A thread inside a drop's read ends after the read's
/// timeout, five seconds at most.
impl Drop for Drops {
    fn drop(&mut self) {
        self.stop.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The paths dropped since the last take, shared with the drop thread.
#[derive(Clone, Default)]
struct Dropped(Arc<Mutex<Vec<PathBuf>>>);

impl Dropped {
    fn take(&self) -> Vec<PathBuf> {
        std::mem::take(&mut self.0.lock().expect("drops lock"))
    }
}

/// Starts the drop thread on winit's display for the window owning
/// `surface`; None when the thread cannot start. `display` and
/// `surface` are the raw handles winit exposes, valid for the window's
/// life, and the handle returned is dropped before the window.
pub fn start(display: *mut c_void, surface: *mut c_void, wake: Waker) -> Option<Drops> {
    // SAFETY: winit opened this display through libwayland and keeps it
    // open while the window lives; the connection made here never
    // disconnects it, and the thread ends before the window is dropped.
    let backend = unsafe { Backend::from_foreign_display(display.cast()) };
    let conn = Connection::from_backend(backend);
    // SAFETY: the pointer is winit's live wl_surface proxy for the
    // window, whose id alone is read here.
    let surface = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.cast()) }.ok()?;
    let (stop, stop_end) = UnixStream::pair().ok()?;
    let dropped = Dropped::default();
    let shared = dropped.clone();
    let thread = std::thread::Builder::new()
        .name("wayland-drops".to_string())
        .spawn(move || {
            let mut queue = conn.new_event_queue();
            let qh = queue.handle();
            let _registry = conn.display().get_registry(&qh, ());
            let mut state = State {
                conn: conn.clone(),
                surface,
                manager: None,
                seats: Vec::new(),
                devices: Vec::new(),
                offers: HashMap::new(),
                dragging: None,
                drops: shared,
                wake,
            };
            let end = serve(&conn, &mut queue, &mut state, &stop_end);
            finish(end, (queue, state, conn));
        })
        .ok()?;
    Some(Drops {
        dropped,
        stop: Some(stop),
        thread: Some(thread),
    })
}

/// The thread's loop: the pending events dispatched, the requests
/// flushed, then a wait for the display's data or the stop. It returns
/// on the stop and on a failure of the display, saying which, the read
/// cancelled.
fn serve(
    conn: &Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
    stop: &UnixStream,
) -> End {
    loop {
        if queue.dispatch_pending(state).is_err() {
            return End::Failed;
        }
        if let Err(err) = conn.flush() {
            if !would_block(&err) {
                return End::Failed;
            }
        }
        // None: events were queued meanwhile, dispatched at the next turn.
        let Some(guard) = conn.prepare_read() else {
            continue;
        };
        match wake(guard.connection_fd(), stop.as_fd()) {
            Wake::Display => {
                if let Err(err) = guard.read() {
                    if !would_block(&err) {
                        return End::Failed;
                    }
                }
            }
            // The guard's drop cancels the read.
            Wake::Stop => return End::Stop,
            Wake::Failed => return End::Failed,
        }
    }
}

fn would_block(err: &WaylandError) -> bool {
    matches!(err, WaylandError::Io(e) if e.kind() == ErrorKind::WouldBlock)
}

/// How the thread's loop ended: on the stop, or on a failure of the
/// display.
#[derive(Debug, PartialEq, Eq)]
enum End {
    Stop,
    Failed,
}

/// What the thread does with its queue and its objects at the end. On
/// the stop the display is alive and they drop. On a failure the
/// process is ending and the display may be in any state: a queue
/// still holding undispatched events would make libwayland print a
/// warning on the error output, so they are forgotten, a few kilobytes
/// the system takes back.
fn finish<T>(end: End, held: T) {
    match end {
        End::Stop => drop(held),
        End::Failed => std::mem::forget(held),
    }
}

/// What ended the wait on the display's descriptor and the stop pipe.
#[derive(Debug, PartialEq, Eq)]
enum Wake {
    Display,
    Stop,
    Failed,
}

/// Waits until the display has data, or the stop pipe is closed or
/// written, whichever comes first; the stop wins a tie.
fn wake(display: BorrowedFd<'_>, stop: BorrowedFd<'_>) -> Wake {
    let mut fds = [
        PollFd::new(&display, PollFlags::IN | PollFlags::ERR),
        PollFd::new(&stop, PollFlags::IN),
    ];
    loop {
        match poll(&mut fds, None) {
            Ok(_) => break,
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return Wake::Failed,
        }
    }
    if !fds[1].revents().is_empty() {
        Wake::Stop
    } else if !fds[0].revents().is_empty() {
        Wake::Display
    } else {
        Wake::Failed
    }
}

struct State {
    conn: Connection,
    /// The window's surface; drags over any other surface are ignored.
    surface: ObjectId,
    manager: Option<WlDataDeviceManager>,
    seats: Vec<WlSeat>,
    /// One data device per seat, made once the manager and the seat
    /// are both bound.
    devices: Vec<WlDataDevice>,
    /// The types each announced offer carries, filled as they arrive.
    offers: HashMap<ObjectId, Vec<String>>,
    /// The offer of the drag over the window, when it carries a list of
    /// files; accepted at enter, read at drop.
    dragging: Option<WlDataOffer>,
    drops: Dropped,
    wake: Waker,
}

impl State {
    fn attach_devices(&mut self, qh: &QueueHandle<State>) {
        let Some(manager) = self.manager.as_ref() else {
            return;
        };
        for seat in &self.seats[self.devices.len()..] {
            self.devices.push(manager.get_data_device(seat, qh, ()));
        }
    }

    fn forget_offer(&mut self, offer: &WlDataOffer) {
        self.offers.remove(&offer.id());
        offer.destroy();
    }

    /// Reads the dropped list through a socket pair: the source writes
    /// into one end and closes it, this end reads to the close. A
    /// source that never writes releases the thread after the timeout.
    fn receive(&mut self, offer: &WlDataOffer) -> Vec<PathBuf> {
        let Ok((mut read, write)) = UnixStream::pair() else {
            return Vec::new();
        };
        offer.receive(URI_LIST.to_string(), write.as_fd());
        drop(write);
        let _ = self.conn.flush();
        let _ = read.set_read_timeout(Some(Duration::from_secs(5)));
        let mut text = String::new();
        let _ = read.read_to_string(&mut text);
        parse_uri_list(&text)
    }
}

/// The local files a `text/uri-list` names, in order: `file:` URIs on
/// this host, percent escapes decoded; comment lines and other schemes
/// are skipped.
pub fn parse_uri_list(text: &str) -> Vec<PathBuf> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let rest = line.strip_prefix("file://")?;
            let path = match rest.find('/') {
                Some(0) => rest,
                Some(slash) if rest[..slash].eq_ignore_ascii_case("localhost") => &rest[slash..],
                _ => return None,
            };
            Some(PathBuf::from(
                crate::doc::html::percent_decode(path).into_owned(),
            ))
        })
        .collect()
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut State,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<State>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_data_device_manager" => {
                    state.manager = Some(registry.bind(name, version.min(3), qh, ()));
                    state.attach_devices(qh);
                }
                "wl_seat" => {
                    state
                        .seats
                        .push(registry.bind(name, version.min(1), qh, ()));
                    state.attach_devices(qh);
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        _: &mut State,
        _: &WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
    }
}

impl Dispatch<WlDataDeviceManager, ()> for State {
    fn event(
        _: &mut State,
        _: &WlDataDeviceManager,
        _: <WlDataDeviceManager as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
    }
}

impl Dispatch<WlDataDevice, ()> for State {
    fn event(
        state: &mut State,
        _: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        match event {
            wl_data_device::Event::DataOffer { id } => {
                state.offers.insert(id.id(), Vec::new());
            }
            wl_data_device::Event::Enter {
                serial,
                surface,
                id: Some(offer),
                ..
            } => {
                let ours = surface.id() == state.surface;
                let files = state
                    .offers
                    .get(&offer.id())
                    .is_some_and(|types| types.iter().any(|t| t == URI_LIST));
                if ours && files {
                    // The source needs an accepted type and an action
                    // before it lets the drop happen.
                    offer.accept(serial, Some(URI_LIST.to_string()));
                    if offer.version() >= 3 {
                        offer.set_actions(DndAction::Copy, DndAction::Copy);
                    }
                    state.dragging = Some(offer);
                } else {
                    offer.accept(serial, None);
                    state.forget_offer(&offer);
                }
            }
            wl_data_device::Event::Leave => {
                if let Some(offer) = state.dragging.take() {
                    state.forget_offer(&offer);
                }
            }
            wl_data_device::Event::Drop => {
                if let Some(offer) = state.dragging.take() {
                    let paths = state.receive(&offer);
                    if offer.version() >= 3 {
                        offer.finish();
                    }
                    state.forget_offer(&offer);
                    if !paths.is_empty() {
                        state.drops.0.lock().expect("drops lock").extend(paths);
                        (state.wake)();
                    }
                }
            }
            // The clipboard's offers arrive on the same device and are
            // not this module's to hold.
            wl_data_device::Event::Selection { id: Some(offer) } => state.forget_offer(&offer),
            _ => {}
        }
    }

    event_created_child!(State, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ()),
    ]);
}

impl Dispatch<WlDataOffer, ()> for State {
    fn event(
        state: &mut State,
        offer: &WlDataOffer,
        event: wl_data_offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = event {
            if let Some(types) = state.offers.get_mut(&offer.id()) {
                types.push(mime_type);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uri_list_yields_local_files_in_order() {
        let text = "# dropped from the file manager\r\n\
                    file:///home/me/notes/a%20b.md\r\n\
                    file://localhost/home/me/c.epub\r\n\
                    https://x.tld/not-a-file.md\r\n\
                    file://otherhost/home/me/d.md\r\n\
                    \r\n\
                    file:///home/me/caf%C3%A9.txt";
        assert_eq!(
            parse_uri_list(text),
            [
                PathBuf::from("/home/me/notes/a b.md"),
                PathBuf::from("/home/me/c.epub"),
                PathBuf::from("/home/me/café.txt"),
            ]
        );
        assert!(parse_uri_list("").is_empty());
        assert!(parse_uri_list("file://").is_empty());
    }

    #[test]
    fn the_wait_ends_on_the_stop() {
        let (display, _far) = UnixStream::pair().expect("a pair");
        let (stop, app_end) = UnixStream::pair().expect("a pair");
        drop(app_end);
        assert_eq!(wake(display.as_fd(), stop.as_fd()), Wake::Stop);
    }

    #[test]
    fn the_wait_ends_when_the_display_has_data() {
        use std::io::Write;
        let (display, mut far) = UnixStream::pair().expect("a pair");
        let (stop, _app_end) = UnixStream::pair().expect("a pair");
        far.write_all(b"event").expect("written");
        assert_eq!(wake(display.as_fd(), stop.as_fd()), Wake::Display);
    }

    /// On a failure of the display the process is ending and the
    /// display may be in any state: the objects are forgotten, not
    /// dropped. On the stop the display is alive and they drop.
    #[test]
    fn a_failure_of_the_display_forgets_the_objects_and_the_stop_drops_them() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct Held(Arc<AtomicBool>);
        impl Drop for Held {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let dropped = Arc::new(AtomicBool::new(false));
        finish(End::Stop, Held(Arc::clone(&dropped)));
        assert!(dropped.load(Ordering::SeqCst), "the stop drops the objects");
        let dropped = Arc::new(AtomicBool::new(false));
        finish(End::Failed, Held(Arc::clone(&dropped)));
        assert!(!dropped.load(Ordering::SeqCst), "a failure forgets them");
    }

    #[test]
    fn the_stop_wins_when_both_are_ready() {
        use std::io::Write;
        let (display, mut far) = UnixStream::pair().expect("a pair");
        let (stop, app_end) = UnixStream::pair().expect("a pair");
        far.write_all(b"event").expect("written");
        drop(app_end);
        assert_eq!(wake(display.as_fd(), stop.as_fd()), Wake::Stop);
    }
}
