//! The development runtime (feature `dev`): the dev server's bundles with
//! hot updates, its errors over the app and the app's logs in the
//! developer's terminal, as on Android (`nsh-platform-gpui`'s dev client).
//!
//! A TV reaches the developer's machine through `sdb`, not the network:
//!
//! - first through a fixed local port, [`DEV_PORT`], which `nsh run` maps to
//!   the dev server on the machine with `sdb reverse` (the app connects to
//!   its own `127.0.0.1`, so no URL has to reach it);
//! - and when that port refuses [`FALLBACK_AFTER`] times in a row (a TV
//!   without `sdb reverse`, or the mapping gone), also by listening on
//!   [`LISTEN_PORT`] of its loopback, which `nsh run` maps from the machine
//!   with `sdb forward` and connects to, relaying to the dev server (`sdbd`
//!   connects there locally; no other host on the TV's network can). From
//!   then on both are tried in turn, each connection the dev client's next
//!   ([`Connector`]); the session speaks the same protocol over either.
//!
//! The assets come from the dev server over HTTP at the same port, which
//! only `sdb reverse` reaches: over `sdb forward` the bundles, hot updates
//! and logs work, and `asset:/` files load once the reverse mapping does.
//!
//! Logs go to the dev server while connected and to the system's log
//! (dlog) otherwise.

use std::{
    io,
    net::{SocketAddr, TcpListener, TcpStream},
    time::{Duration, Instant},
};

use nsh_platform_gpui::AppView;

/// The port on the TV that `sdb reverse` maps to the dev server.
pub(crate) const DEV_PORT: u16 = 8790;
/// The port the TV listens on for `sdb forward`.
pub(crate) const LISTEN_PORT: u16 = 8791;
/// The dev server's URL as the app sees it (through `sdb reverse`).
#[cfg_attr(not(target_vendor = "tizen"), allow(dead_code))]
pub(crate) const URL: &str = "ws://127.0.0.1:8790/ws";
/// Refusals of [`DEV_PORT`] in a row before the TV also listens.
pub(crate) const FALLBACK_AFTER: u32 = 3;
/// How long one connection to [`DEV_PORT`] may take (it is local).
const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);
/// How long one turn waits for the machine on [`LISTEN_PORT`].
const ACCEPT_WAIT: Duration = Duration::from_millis(1500);

/// A way to the dev server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    /// Connect to the reverse-mapped port.
    Reverse,
    /// Take the connection the machine makes to the forwarded port.
    Forward,
}

/// Which way the next connection is tried: [`Route::Reverse`] alone until
/// it refused [`FALLBACK_AFTER`] times in a row, then both in turn,
/// starting with the one that worked last.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Policy {
    /// Refusals of the reverse port in a row, before listening.
    refused: u32,
    /// Listening for `sdb forward` (for good, once begun).
    listening: bool,
    /// The route of the last try.
    last: Option<Route>,
}

impl Policy {
    /// Whether the TV listens on [`LISTEN_PORT`].
    #[cfg(test)]
    fn listening(&self) -> bool {
        self.listening
    }

    /// The route to try next.
    pub(crate) fn next(&mut self) -> Route {
        let route = match (self.listening, self.last) {
            (false, _) => Route::Reverse,
            (true, Some(Route::Reverse)) => Route::Forward,
            (true, _) => Route::Reverse,
        };
        self.last = Some(route);
        route
    }

    /// The try of `route` failed.
    pub(crate) fn failed(&mut self, route: Route) {
        if route == Route::Reverse && !self.listening {
            self.refused += 1;
            if self.refused >= FALLBACK_AFTER {
                self.listening = true;
            }
        }
    }

    /// The try of `route` connected: when the session ends, it is tried
    /// first again (the other one next).
    pub(crate) fn connected(&mut self, route: Route) {
        self.refused = 0;
        // `next` alternates from the route before the one to try first.
        self.last = Some(match route {
            Route::Reverse => Route::Forward,
            Route::Forward => Route::Reverse,
        });
    }
}

/// The dev client's connections ([`nsh_dev::DevTransport`]) by [`Policy`].
pub(crate) struct Connector {
    policy: Policy,
    reverse: SocketAddr,
    listen: SocketAddr,
    listener: Option<TcpListener>,
}

impl Connector {
    /// Connects to `reverse` and, once it falls back, listens on `listen`.
    pub(crate) fn new(reverse: SocketAddr, listen: SocketAddr) -> Connector {
        Connector {
            policy: Policy::default(),
            reverse,
            listen,
            listener: None,
        }
    }

    /// The TV's ports, both on its loopback: [`DEV_PORT`], and
    /// [`LISTEN_PORT`], where `sdbd` delivers the connections `sdb forward`
    /// makes. Never another interface: a host on the TV's network could
    /// then connect in the dev server's place and run its code in the app.
    #[cfg_attr(not(target_vendor = "tizen"), allow(dead_code))]
    pub(crate) fn tv() -> Connector {
        Connector::new(
            SocketAddr::from(([127, 0, 0, 1], DEV_PORT)),
            SocketAddr::from(([127, 0, 0, 1], LISTEN_PORT)),
        )
    }

    fn try_route(&mut self, route: Route) -> io::Result<TcpStream> {
        match route {
            Route::Reverse => TcpStream::connect_timeout(&self.reverse, CONNECT_TIMEOUT),
            Route::Forward => {
                if self.listener.is_none() {
                    let listener = TcpListener::bind(self.listen)?;
                    listener.set_nonblocking(true)?;
                    log::info!(
                        "nsh::dev: no dev server on 127.0.0.1:{} (sdb reverse); listening on port {} for sdb forward",
                        self.reverse.port(),
                        listener.local_addr()?.port()
                    );
                    self.listener = Some(listener);
                }
                let listener = self.listener.as_ref().expect("bound above");
                accept_within(listener, ACCEPT_WAIT)
            }
        }
    }
}

/// The first connection `listener` (non-blocking) takes within `wait`.
fn accept_within(listener: &TcpListener, wait: Duration) -> io::Result<TcpStream> {
    let until = Instant::now() + wait;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                return Ok(stream);
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= until {
                    return Err(io::ErrorKind::TimedOut.into());
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(err) => return Err(err),
        }
    }
}

impl nsh_dev::DevTransport for Connector {
    fn connect(&mut self) -> io::Result<TcpStream> {
        let route = self.policy.next();
        match self.try_route(route) {
            Ok(stream) => {
                self.policy.connected(route);
                Ok(stream)
            }
            Err(err) => {
                self.policy.failed(route);
                Err(err)
            }
        }
    }
}

/// Connects `view` to the dev server at `url` through the TV's ports.
pub(crate) fn connect(view: &AppView, url: String, cx: &mut gpui::Context<AppView>) {
    view.connect_dev_server_with(url, Box::new(Connector::tv()), cx);
}

/// Logs what the development hosts show (NSH's, the app's `js`, and
/// warnings and errors of everything else) to the dev server while
/// connected, and everything else, up to info, to the system's log.
struct DevLogger;

/// Whether the developer's terminal shows `metadata`'s lines.
fn shown(metadata: &log::Metadata<'_>) -> bool {
    metadata.target().starts_with("nsh")
        || metadata.target() == "js"
        || metadata.level() <= log::Level::Warn
}

impl log::Log for DevLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record<'_>) {
        let forwarded = || shown(record.metadata()) && nsh_platform_gpui::dev::forward(record);
        if self.enabled(record.metadata()) && !forwarded() {
            #[cfg(target_vendor = "tizen")]
            crate::tizen::dlog(record.level(), record.target(), &record.args().to_string());
        }
    }

    fn flush(&self) {}
}

/// Installs [`DevLogger`] (before the platform installs its own).
#[cfg_attr(not(target_vendor = "tizen"), allow(dead_code))]
pub(crate) fn init_logging() {
    static LOGGER: DevLogger = DevLogger;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

#[cfg(test)]
mod tests {
    use nsh_dev::DevTransport as _;

    use super::*;

    #[test]
    fn the_reverse_port_alone_until_it_refused_three_times() {
        let mut policy = Policy::default();
        for _ in 0..FALLBACK_AFTER - 1 {
            assert_eq!(policy.next(), Route::Reverse);
            policy.failed(Route::Reverse);
            assert!(!policy.listening());
        }
        assert_eq!(policy.next(), Route::Reverse);
        policy.failed(Route::Reverse);
        assert!(policy.listening());
        // Then both in turn.
        assert_eq!(policy.next(), Route::Forward);
        policy.failed(Route::Forward);
        assert_eq!(policy.next(), Route::Reverse);
        policy.failed(Route::Reverse);
        assert_eq!(policy.next(), Route::Forward);
    }

    #[test]
    fn a_connection_resets_the_count_and_is_tried_first_again() {
        let mut policy = Policy::default();
        policy.next();
        policy.failed(Route::Reverse);
        policy.next();
        policy.connected(Route::Reverse);
        // The session ended: the count starts again.
        for _ in 0..FALLBACK_AFTER - 1 {
            assert_eq!(policy.next(), Route::Reverse);
            policy.failed(Route::Reverse);
        }
        assert!(!policy.listening());
        // Once listening, the forwarded route that worked comes first.
        let mut policy = Policy::default();
        for _ in 0..FALLBACK_AFTER {
            policy.next();
            policy.failed(Route::Reverse);
        }
        assert_eq!(policy.next(), Route::Forward);
        policy.connected(Route::Forward);
        assert_eq!(policy.next(), Route::Forward);
        policy.failed(Route::Forward);
        assert_eq!(policy.next(), Route::Reverse);
        // A failed forward try never counts towards anything.
        assert!(policy.listening());
    }

    #[test]
    fn the_tv_listens_on_its_loopback_only() {
        let tv = Connector::tv();
        assert_eq!(tv.reverse, SocketAddr::from(([127, 0, 0, 1], DEV_PORT)));
        assert!(tv.listen.ip().is_loopback(), "{}", tv.listen);
        assert_eq!(tv.listen.port(), LISTEN_PORT);
    }

    /// A free port of this machine's loopback (closed again).
    fn free_port() -> SocketAddr {
        TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
    }

    #[test]
    fn the_reverse_port_connects_when_something_listens_there() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut connector = Connector::new(server.local_addr().unwrap(), free_port());
        assert!(connector.connect().is_ok());
        assert!(server.accept().is_ok());
        assert!(!connector.policy.listening());
    }

    /// Nothing on the reverse port: after three refusals the TV listens,
    /// and the machine's connection there (what `sdb forward` makes) is
    /// the next one.
    #[test]
    fn refused_three_times_it_takes_the_machines_connection() {
        let listen = free_port();
        let mut connector = Connector::new(free_port(), listen);
        for _ in 0..FALLBACK_AFTER {
            assert!(connector.connect().is_err());
        }
        assert!(connector.policy.listening());
        let machine = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                if let Ok(stream) = TcpStream::connect(listen) {
                    return stream;
                }
                assert!(Instant::now() < until, "the TV never listened");
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        let stream = connector.connect().expect("the forwarded connection");
        let machine = machine.join().unwrap();
        assert_eq!(stream.peer_addr().unwrap(), machine.local_addr().unwrap());
        // Its socket blocks, as the dev client reads it.
        assert!(stream.set_read_timeout(None).is_ok());
    }
}
