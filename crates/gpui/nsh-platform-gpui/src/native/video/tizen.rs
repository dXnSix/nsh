//! Videos on a Samsung TV: dXn.GpuiTizen's `VideoPlayer` (Tizen's
//! `capi-media-player`), with NSH's own events and control bar.
//!
//! - **Output.** The player's pictures go to the TV's video plane under the
//!   window, seen through a hole the view paints (`Output::Plane`:
//!   hardware decoding, no copies), while the view is drawn as it is laid
//!   out: no rotation or skew, fully opaque, no color filter, and not cut
//!   by a clip. Otherwise (a carousel's turning card, a fading or clipped
//!   view) its frames are drawn as a texture (`Output::Texture`), as other
//!   content is. The output is chosen where the video is first drawn
//!   ([`output`]); a video on the plane that comes to need a texture
//!   (its view starts to turn) opens again as one, at the time it was at.
//!   The platform takes the texture where its window cannot turn
//!   translucent.
//! - **Props and commands** ([`ops`]): a new source opens a player (it
//!   prepares at once; `paused: false` starts it when it is ready),
//!   `paused`, `muted`, `volume` and `repeat` follow, `seek(seconds)` seeks.
//!   The player seeks only once prepared: a seek before that, and the time
//!   a video opened again as a texture was at, wait for it, playing after
//!   them ([`Gate`]). `rate` has no public native API on Tizen 8.0: it
//!   stays 1.
//! - **Interruptions** ([`Gate`]): when the system takes the decoder (another
//!   app, typically around a pause and resume), the player may be left
//!   unprepared, or paused while the props say play. The app hears
//!   `onBuffer` with `true`; once the app is active again the player is
//!   prepared again if it has to be, given its props, sought to where it
//!   was and played as `paused` says, and the app hears `onBuffer` with
//!   `false`. When it cannot be prepared again, `onError`.
//! - **Events** ([`Events`]): `onLoadStart` as a source opens, `onLoad`
//!   once it is ready (its duration; the picture's size is the player's,
//!   not reported), `onProgress` every `progressUpdateInterval` while
//!   playing, `onBuffer` as buffering starts and ends, `onEnd` (a looping
//!   video loops in the player instead), `onError` with the player's
//!   error.
//! - **Controls**: `controls` draws NSH's control bar (play / pause, a
//!   scrubber, the time) over the picture, as on the desktops.

#[cfg(target_vendor = "tizen")]
pub(crate) use item::VideoItem;
use nsh_core::{EventKind, EventPayload, PayloadValue, VideoProps};

use super::finite;

/// Where a player's pictures go (the platform's `video::Output`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Output {
    /// The TV's video plane under the window, through a hole.
    Plane,
    /// A texture of its frames, drawn as content.
    Texture,
}

/// What the player reported (the platform's `video::Notice`, which the
/// item turns into this).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Notice {
    /// Prepared: the duration is known.
    Ready,
    Playing,
    Paused,
    /// Every seek asked for is done.
    Seeked,
    /// The end was reached (not looping).
    Ended,
    /// The player's error.
    Failed(String),
    /// Buffering, 0 to 100.
    Buffering(u8),
    /// The system interrupted it (another app took the decoder), leaving
    /// it as [`Left`] says.
    Interrupted(Left),
    /// A new frame for the texture.
    Frame,
}

/// What an interruption by the system left the player in (the platform's
/// `Notice::Interrupted` state).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Left {
    /// Unprepared (idle): only preparing it again brings it back.
    Lost,
    /// Prepared, not playing (paused or stopped).
    Paused,
    /// Playing, or still preparing: nothing was lost.
    Kept,
}

/// What the item calls on its player, or tells the app.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Call {
    /// One of the props' calls ([`ops`]; never `Open` or `Close`).
    Op(Op),
    /// Seek to the time, in seconds.
    Seek(f64),
    /// Prepare the source again (`VideoPlayer::prepare`).
    Prepare,
    /// The video stalled (`onBuffer` with `true`) or goes on (`false`).
    Stalled(bool),
}

/// How an interrupted player comes back once the app is active.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Recovery {
    /// Prepare again (the system unprepared it).
    prepare: bool,
    /// Where to continue (seconds): the time it was at, or sought since.
    at: f64,
    /// A seek was asked for while it was interrupted.
    sought: bool,
    /// Whether to play: the props' `paused` (or the control bar) since.
    play: bool,
}

/// The calls a player takes as it can. A player prepares as it opens and
/// seeks only once prepared (`Notice::Ready`): a seek before that waits
/// for it, the start position of a video opened again as a texture too,
/// and playing waits behind it, so the video plays from there rather than
/// from the start. When the system interrupts the player and it lost what
/// it played (unprepared, or paused while the props say play), the app
/// hears that the video stalled (`onBuffer`), and once the app is active it
/// is prepared again if it has to be, given its props again (muted,
/// volume, looping), sought to where it was and played as the props say;
/// the app hears that it goes on when it does.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Gate {
    /// Prepared: seeks reach the player.
    prepared: bool,
    /// The seek waiting for it to be prepared.
    seek: Option<f64>,
    /// Play (`true`) or not once prepared, after that seek.
    then: Option<bool>,
    /// Interrupted: what brings it back.
    interrupted: Option<Recovery>,
    /// Prepared again after an interruption: the app hears that the video
    /// goes on once it is ready.
    recovering: bool,
}

impl Gate {
    /// The props' call `op` (not `Open` or `Close`): now, or (playing and
    /// pausing) once the player is prepared or back.
    pub(crate) fn op(&mut self, op: Op) -> Vec<Call> {
        let play = match op {
            Op::Play => true,
            Op::Pause => false,
            op => return vec![Call::Op(op)],
        };
        if let Some(recovery) = &mut self.interrupted {
            recovery.play = play;
            return Vec::new();
        }
        if !self.prepared {
            self.then = Some(play);
            return Vec::new();
        }
        vec![Call::Op(op)]
    }

    /// `seek(seconds)`: now, or once the player is prepared or back.
    pub(crate) fn seek(&mut self, at: f64) -> Vec<Call> {
        if let Some(recovery) = &mut self.interrupted {
            recovery.at = at;
            recovery.sought = true;
            return Vec::new();
        }
        if !self.prepared {
            self.seek = Some(at);
            return Vec::new();
        }
        vec![Call::Seek(at)]
    }

    /// The player is prepared (`Notice::Ready`): what waited for it.
    pub(crate) fn ready(&mut self) -> Vec<Call> {
        self.prepared = true;
        let mut calls = Vec::new();
        if let Some(recovery) = &mut self.interrupted {
            // A prepare from before the interruption came through: it
            // need not prepare again, but it is at the start; the rest
            // waits for the app.
            if std::mem::take(&mut recovery.prepare) {
                recovery.sought = true;
            }
            return calls;
        }
        calls.extend(self.seek.take().map(Call::Seek));
        if self.then.take() == Some(true) {
            calls.push(Call::Op(Op::Play));
        }
        if std::mem::take(&mut self.recovering) {
            calls.push(Call::Stalled(false));
        }
        calls
    }

    /// The system interrupted the player, leaving it as `left` says, at
    /// time `at`; `play` is whether the props (or the control bar) want it
    /// playing.
    pub(crate) fn interrupted(&mut self, left: Left, play: bool, at: f64) -> Vec<Call> {
        let prepare = left == Left::Lost;
        if prepare {
            self.prepared = false;
        }
        if let Some(recovery) = &mut self.interrupted {
            recovery.prepare |= prepare;
            return Vec::new();
        }
        let lost = match left {
            Left::Lost => true,
            Left::Paused => play,
            Left::Kept => false,
        };
        if !lost {
            return Vec::new();
        }
        // Where it was is where it goes on: what waited goes with it.
        let at = self.seek.take().unwrap_or(at);
        let play = self.then.take().unwrap_or(play);
        self.recovering = false;
        self.interrupted = Some(Recovery {
            prepare,
            at,
            sought: false,
            play,
        });
        vec![Call::Stalled(true)]
    }

    /// The player is replaced by a new one (a video on the plane opened
    /// again as a texture): what waited goes with the old one (the item
    /// gives the new one the time and the props), but a stall the app
    /// heard of ends once the new one is ready. Whether the old one waited
    /// to come back (the new one plays as the props say, then).
    pub(crate) fn reopen(&mut self) -> bool {
        let waited = self.interrupted.is_some();
        *self = Gate {
            recovering: waited || self.recovering,
            ..Gate::default()
        };
        waited
    }

    /// Whether an interrupted player waits for the app to be active.
    pub(crate) fn waits(&self) -> bool {
        self.interrupted.is_some()
    }

    /// The app is active: what brings an interrupted player back, with
    /// `props` (the video's) given to it again when it is prepared again.
    pub(crate) fn resume(&mut self, props: Option<&VideoProps>) -> Vec<Call> {
        let Some(recovery) = self.interrupted.take() else {
            return Vec::new();
        };
        if recovery.prepare {
            let mut calls = vec![Call::Prepare];
            calls.extend(
                props
                    .map(|p| ops(None, p))
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|op| matches!(op, Op::Muted(_) | Op::Volume(_) | Op::Looping(_)))
                    .map(Call::Op),
            );
            self.prepared = false;
            self.seek = Some(recovery.at);
            self.then = Some(recovery.play);
            self.recovering = true;
            return calls;
        }
        let mut calls = Vec::new();
        if recovery.sought {
            calls.push(Call::Seek(recovery.at));
        }
        if recovery.play {
            calls.push(Call::Op(Op::Play));
        }
        calls.push(Call::Stalled(false));
        calls
    }
}

/// What the view's drawing does to the video where it is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placement {
    /// The transform keeps rectangles axis-aligned (a translation and a
    /// scale).
    pub(crate) axis_aligned: bool,
    /// The opacity it is drawn with (the view's and its ancestors').
    pub(crate) opacity: f32,
    /// A color filter applies.
    pub(crate) filtered: bool,
    /// A clip cuts the picture (an ancestor's bounds, rounded corners).
    pub(crate) clipped: bool,
}

/// The output to ask the platform for at `placement`: `None` (the video
/// plane where the window can show it) for a video drawn as laid out,
/// the texture otherwise.
pub(crate) fn output(placement: &Placement) -> Option<Output> {
    let plain = placement.axis_aligned
        && placement.opacity >= 1.0
        && !placement.filtered
        && !placement.clipped;
    (!plain).then_some(Output::Texture)
}

/// Whether a player drawing to `current` opens again for `wanted`: a video
/// on the plane whose view now needs the texture (never back: the texture
/// shows everywhere).
pub(crate) fn reopen(current: Output, wanted: Option<Output>) -> bool {
    current == Output::Plane && wanted == Some(Output::Texture)
}

/// What the player is told.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Op {
    /// Open the source (replacing the player, `onLoadStart`).
    Open(std::sync::Arc<str>),
    /// No source: the player goes.
    Close,
    Play,
    Pause,
    Muted(bool),
    /// 0 to 1.
    Volume(f32),
    Looping(bool),
}

/// The player's ops for new props `new` over `old` (`None`: the first).
pub(crate) fn ops(old: Option<&VideoProps>, new: &VideoProps) -> Vec<Op> {
    let volume = |p: &VideoProps| {
        if p.volume.is_finite() {
            p.volume.clamp(0.0, 1.0)
        } else {
            1.0
        }
    };
    let opened = old.is_none_or(|old| old.source != new.source);
    if opened {
        let Some(source) = new.source.clone() else {
            return match old {
                Some(_) => vec![Op::Close],
                None => Vec::new(),
            };
        };
        let mut ops = vec![
            Op::Open(source),
            Op::Muted(new.muted),
            Op::Volume(volume(new)),
            Op::Looping(new.repeat),
        ];
        if !new.paused {
            ops.push(Op::Play);
        }
        return ops;
    }
    let Some(old) = old else {
        return Vec::new();
    };
    if new.source.is_none() {
        return Vec::new();
    }
    let mut ops = Vec::new();
    if old.paused != new.paused {
        ops.push(if new.paused { Op::Pause } else { Op::Play });
    }
    if old.muted != new.muted {
        ops.push(Op::Muted(new.muted));
    }
    if volume(old) != volume(new) {
        ops.push(Op::Volume(volume(new)));
    }
    if old.repeat != new.repeat {
        ops.push(Op::Looping(new.repeat));
    }
    ops
}

/// An event for the app (or the view to draw again).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Emit {
    /// `onLoad`, with the duration in seconds.
    Load(f64),
    End,
    Error(String),
    Buffer(bool),
    /// Draw the view again (a new frame, playback started or stopped).
    Redraw,
}

impl Emit {
    /// The event's kind and payload, for those the app hears.
    pub(crate) fn event(&self) -> Option<(EventKind, EventPayload)> {
        Some(match self {
            Emit::Load(duration) => (
                EventKind::Load,
                EventPayload::Map(vec![
                    ("duration", PayloadValue::Number(finite(*duration))),
                    ("width", PayloadValue::Number(0.0)),
                    ("height", PayloadValue::Number(0.0)),
                ]),
            ),
            Emit::End => (EventKind::End, EventPayload::None),
            Emit::Error(message) => (EventKind::Error, crate::native::error(message.clone())),
            Emit::Buffer(buffering) => (
                EventKind::Buffer,
                EventPayload::Map(vec![("isBuffering", PayloadValue::Bool(*buffering))]),
            ),
            Emit::Redraw => return None,
        })
    }
}

/// What one source reported, so each event goes once.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Events {
    loaded: bool,
    buffering: bool,
    failed: bool,
    /// Playing, as the player last said.
    pub(crate) playing: bool,
    /// The end was reached.
    pub(crate) ended: bool,
}

impl Events {
    /// The events of the player's `notice` for source `source`;
    /// `duration` asks the player for its duration (seconds).
    pub(crate) fn notice(
        &mut self,
        notice: Notice,
        source: &str,
        duration: impl FnOnce() -> f64,
    ) -> Vec<Emit> {
        match notice {
            Notice::Ready if !self.loaded => {
                self.loaded = true;
                vec![Emit::Load(duration()), Emit::Redraw]
            }
            Notice::Ready => Vec::new(),
            Notice::Playing => {
                self.playing = true;
                self.ended = false;
                vec![Emit::Redraw]
            }
            Notice::Paused => {
                self.playing = false;
                vec![Emit::Redraw]
            }
            Notice::Seeked | Notice::Frame => vec![Emit::Redraw],
            Notice::Ended => {
                self.playing = false;
                self.ended = true;
                vec![Emit::End, Emit::Redraw]
            }
            Notice::Failed(error) if !self.failed => {
                self.failed = true;
                self.playing = false;
                vec![Emit::Error(format!("{source}: {error}")), Emit::Redraw]
            }
            Notice::Failed(_) => Vec::new(),
            Notice::Buffering(percent) => {
                let buffering = percent < 100;
                if std::mem::replace(&mut self.buffering, buffering) == buffering {
                    return Vec::new();
                }
                vec![Emit::Buffer(buffering)]
            }
            Notice::Interrupted(left) => {
                self.playing &= left == Left::Kept;
                vec![Emit::Redraw]
            }
        }
    }

    /// The video stalled after an interruption, or goes on ([`Gate`]):
    /// `onBuffer`, unless the app heard it already.
    pub(crate) fn stalled(&mut self, stalled: bool) -> Vec<Emit> {
        if std::mem::replace(&mut self.buffering, stalled) == stalled {
            return Vec::new();
        }
        vec![Emit::Buffer(stalled)]
    }

    /// `onProgress` is due: playing a source that loaded.
    pub(crate) fn progresses(&self) -> bool {
        self.loaded && self.playing && !self.failed
    }
}

/// The `VideoItem` of a TV: a `VideoPlayer` entity, opened where the video
/// is first drawn.
#[cfg(target_vendor = "tizen")]
mod item {
    use std::{
        cell::{Cell, RefCell},
        rc::{Rc, Weak},
        time::Duration,
    };

    use dxn_gpui_tizen::video::{self as platform, VideoPlayer};
    use gpui::{AppContext as _, Entity, Subscription};
    use nsh_core::{
        EventKind, EventPayload, NodeId, PayloadValue, VideoProps, platform::Lifecycle,
    };

    use super::{
        Call, Emit, Events, Gate, Left, Notice, Op, Output, Placement, ops, output, reopen,
    };
    use crate::{
        image::AssetBase,
        input::{Cursor, PointerKind},
        native::{
            Emitter, Host, Location, error, locate,
            video::{
                controls::{self, BAR, BUTTON},
                finite,
            },
        },
        render::DrawCx,
        surface::Surface,
        view::{Hosted, ItemPointer},
    };

    /// An open player.
    struct Open {
        player: Entity<VideoPlayer>,
        output: Output,
        _notices: Subscription,
    }

    /// A video on a TV.
    pub(crate) struct VideoItem {
        emitter: Emitter,
        surface: Weak<Surface>,
        id: NodeId,
        /// The props applied (`None` before the first).
        props: RefCell<Option<VideoProps>>,
        /// The source's URI, opened at the next draw while no player is.
        uri: RefCell<Option<String>>,
        player: RefCell<Option<Open>>,
        /// What the player is told once it opens.
        pending: RefCell<Vec<Op>>,
        /// Where to seek once it opens (reopened, or sought before).
        seek_to: Cell<Option<f64>>,
        /// The open player's calls as it takes them (seeks once prepared,
        /// its way back from an interruption).
        gate: RefCell<Gate>,
        /// Waiting for the app to be active to bring an interrupted player
        /// back.
        waiting: Cell<bool>,
        events: RefCell<Events>,
        /// The duration, once ready (seconds).
        duration: Cell<f64>,
        /// The position last read (seconds).
        time: Cell<f64>,
        ticking: Cell<bool>,
        scrubbing: Cell<bool>,
    }

    impl VideoItem {
        pub(crate) fn new(surface: &Rc<Surface>, id: NodeId) -> Rc<Self> {
            Rc::new(VideoItem {
                emitter: Emitter::new(surface, id),
                surface: Rc::downgrade(surface),
                id,
                props: RefCell::new(None),
                uri: RefCell::new(None),
                player: RefCell::new(None),
                pending: RefCell::default(),
                seek_to: Cell::new(None),
                gate: RefCell::default(),
                waiting: Cell::new(false),
                events: RefCell::default(),
                duration: Cell::new(0.0),
                time: Cell::new(0.0),
                ticking: Cell::new(false),
                scrubbing: Cell::new(false),
            })
        }

        /// New props.
        pub(crate) fn set(&self, surface: &Rc<Surface>, p: &VideoProps, base: Option<&AssetBase>) {
            let old = self.props.borrow_mut().replace(p.clone());
            for op in ops(old.as_ref(), p) {
                match op {
                    Op::Open(source) => self.open(&source, base),
                    Op::Close => self.close(),
                    op => self.apply(op),
                }
            }
            surface.invalidate();
        }

        /// A new source: the player goes, the next draw opens one for it.
        fn open(&self, source: &str, base: Option<&AssetBase>) {
            self.close();
            *self.events.borrow_mut() = Events::default();
            self.duration.set(0.0);
            self.time.set(0.0);
            match locate(source, base) {
                Some(Location::Url(url)) => *self.uri.borrow_mut() = Some(url),
                Some(Location::Path(path)) => {
                    *self.uri.borrow_mut() = Some(path.display().to_string())
                }
                None => {
                    let message = format!("{source}: cannot resolve the video source");
                    self.emitter.emit(EventKind::Error, || error(message));
                    return;
                }
            }
            self.emitter
                .emit(EventKind::LoadStart, || EventPayload::None);
        }

        /// The open player.
        fn current(&self) -> Option<Entity<VideoPlayer>> {
            self.player.borrow().as_ref().map(|o| o.player.clone())
        }

        /// The video `id` of the surface `key`, while it is there.
        fn find(key: Option<u64>, id: NodeId) -> Option<(Rc<Surface>, Rc<VideoItem>)> {
            let surface = key.and_then(Surface::by_key)?;
            let item = match surface.native.hosts.borrow().get(&id) {
                Some(Host::Video(item)) => item.clone(),
                _ => return None,
            };
            Some((surface, item))
        }

        /// Makes `calls` on `player` with the app (from where it is not at
        /// hand), unless the player was replaced by then.
        fn later(&self, player: Entity<VideoPlayer>, calls: Vec<Call>) {
            if calls.is_empty() {
                return;
            }
            let key = self.surface.upgrade().map(|s| s.key());
            let id = self.id;
            crate::rt::on_app(move |cx| {
                let Some((_, item)) = Self::find(key, id) else {
                    return;
                };
                if item.current().is_some_and(|current| current == player) {
                    item.call(&player, calls, cx);
                }
            });
        }

        /// Makes `calls` on `player`.
        fn call(&self, player: &Entity<VideoPlayer>, calls: Vec<Call>, cx: &mut gpui::App) {
            for call in calls {
                let result = match call {
                    Call::Op(op) => {
                        let plays = matches!(op, Op::Play);
                        let result = player.update(cx, |p, cx| match op {
                            Op::Play => p.play(cx),
                            Op::Pause => p.pause(cx),
                            Op::Muted(muted) => p.set_muted(muted),
                            Op::Volume(volume) => p.set_volume(volume),
                            Op::Looping(looping) => p.set_looping(looping),
                            Op::Open(_) | Op::Close => Ok(()),
                        });
                        if plays {
                            self.tick();
                        }
                        result
                    }
                    Call::Seek(at) => {
                        player.update(cx, |p, cx| p.seek(Duration::from_secs_f64(at), cx))
                    }
                    Call::Prepare => {
                        if let Err(err) = player.update(cx, |p, cx| p.prepare(cx)) {
                            // Interrupted for good: the app hears it stopped.
                            *self.gate.borrow_mut() = Gate::default();
                            self.stalled(false);
                            let source = self.uri.borrow().clone().unwrap_or_default();
                            let emits = self.events.borrow_mut().notice(
                                Notice::Failed(format!("cannot prepare it again: {err}")),
                                &source,
                                || 0.0,
                            );
                            self.emit(emits);
                            return;
                        }
                        Ok(())
                    }
                    Call::Stalled(stalled) => {
                        self.stalled(stalled);
                        Ok(())
                    }
                };
                if let Err(err) = result {
                    log::debug!("video: {err}");
                }
            }
        }

        /// `onBuffer` for an interruption, unless the app heard it already.
        fn stalled(&self, stalled: bool) {
            let emits = self.events.borrow_mut().stalled(stalled);
            self.emit(emits);
        }

        /// The app's events (and redraws) of `emits`.
        fn emit(&self, emits: Vec<Emit>) {
            for emit in emits {
                if let Emit::Load(duration) = emit {
                    self.duration.set(finite(duration));
                }
                match emit.event() {
                    Some((kind, payload)) => self.emitter.emit(kind, || payload),
                    None => {
                        if let Some(surface) = self.surface.upgrade() {
                            surface.invalidate();
                        }
                    }
                }
            }
        }

        /// Tells the player `op`, or keeps it for when it opens.
        fn apply(&self, op: Op) {
            let Some(player) = self.current() else {
                self.pending.borrow_mut().push(op);
                return;
            };
            let calls = self.gate.borrow_mut().op(op);
            self.later(player, calls);
        }

        /// `seek(seconds)`.
        pub(crate) fn seek(&self, seconds: f64) {
            let seconds = finite(seconds).max(0.0);
            self.time.set(seconds);
            let Some(player) = self.current() else {
                self.seek_to.set(Some(seconds));
                return;
            };
            let calls = self.gate.borrow_mut().seek(seconds);
            self.later(player, calls);
            if let Some(surface) = self.surface.upgrade() {
                surface.invalidate();
            }
        }

        /// Whether the app is active (an interrupted player comes back only
        /// then).
        fn active(surface: &Surface) -> bool {
            surface.environment.get().lifecycle == Lifecycle::Active
        }

        /// Brings an interrupted player back once the app is active,
        /// looking again every half second until it is.
        fn resume_when_active(&self) {
            if self.waiting.replace(true) {
                return;
            }
            let key = self.surface.upgrade().map(|s| s.key());
            let id = self.id;
            crate::rt::after_ms(500, move || {
                crate::rt::on_app(move |cx| {
                    let Some((surface, item)) = Self::find(key, id) else {
                        return;
                    };
                    item.waiting.set(false);
                    let Some(player) = item.current() else {
                        return;
                    };
                    if !item.gate.borrow().waits() {
                        return;
                    }
                    if !Self::active(&surface) {
                        item.resume_when_active();
                        return;
                    }
                    let calls = item.gate.borrow_mut().resume(item.props.borrow().as_ref());
                    item.call(&player, calls, cx);
                });
            });
        }

        /// The video's view was deleted (or its source went): the player
        /// goes.
        pub(crate) fn close(&self) {
            self.player.borrow_mut().take();
            self.uri.borrow_mut().take();
            self.pending.borrow_mut().clear();
            self.seek_to.set(None);
            *self.gate.borrow_mut() = Gate::default();
        }

        /// Opens the player for the source at `wanted` (with `cx`'s app),
        /// telling it what waited.
        fn start(&self, wanted: Option<Output>, cx: &mut DrawCx<'_>) {
            let Some(uri) = self.uri.borrow().clone() else {
                return;
            };
            let request = wanted.map(|_| platform::Output::Texture);
            let player = cx.app.new(|cx| VideoPlayer::new(&uri, request, cx));
            let key = self.surface.upgrade().map(|s| s.key());
            let id = self.id;
            let notices =
                cx.app
                    .subscribe(&player, move |player, notice: &platform::Notice, cx| {
                        let Some((surface, item)) = Self::find(key, id) else {
                            return;
                        };
                        item.notice(&surface, &player, of_platform(*notice), cx);
                    });
            let (output, failed) = {
                let p = player.read(cx.app);
                let output = match p.output() {
                    platform::Output::Overlay => Output::Plane,
                    platform::Output::Texture => Output::Texture,
                };
                (output, p.error().map(str::to_owned))
            };
            *self.player.borrow_mut() = Some(Open {
                player: player.clone(),
                output,
                _notices: notices,
            });
            if let Some(message) = failed {
                self.events
                    .borrow_mut()
                    .notice(Notice::Failed(message.clone()), &uri, || 0.0);
                let message = format!("{uri}: {message}");
                self.emitter.emit(EventKind::Error, || error(message));
                return;
            }
            // What waited for it: audio and looping now; the seek, then
            // playing, once it is prepared.
            let waiting = std::mem::take(&mut *self.pending.borrow_mut());
            let mut calls = Vec::new();
            {
                let mut gate = self.gate.borrow_mut();
                if let Some(at) = self.seek_to.take() {
                    calls.extend(gate.seek(at));
                }
                for op in waiting {
                    calls.extend(gate.op(op));
                }
            }
            self.call(&player, calls, cx.app);
        }

        /// What the player reported.
        fn notice(
            &self,
            surface: &Rc<Surface>,
            player: &Entity<VideoPlayer>,
            notice: Notice,
            cx: &mut gpui::App,
        ) {
            // What waited for the player (it is prepared), or brings it back
            // (it was interrupted).
            let calls = match &notice {
                Notice::Ready => self.gate.borrow_mut().ready(),
                Notice::Interrupted(left) => {
                    if let Ok(time) = player.update(cx, |p, _| p.position()) {
                        self.time.set(time.as_secs_f64());
                    }
                    let play = self.props.borrow().as_ref().is_some_and(|p| !p.paused);
                    let mut calls =
                        self.gate
                            .borrow_mut()
                            .interrupted(*left, play, self.time.get());
                    if self.gate.borrow().waits() {
                        if Self::active(surface) {
                            let props = self.props.borrow().clone();
                            calls.extend(self.gate.borrow_mut().resume(props.as_ref()));
                        } else {
                            self.resume_when_active();
                        }
                    }
                    calls
                }
                _ => Vec::new(),
            };
            let source = self.uri.borrow().clone().unwrap_or_default();
            let emits = self.events.borrow_mut().notice(notice, &source, || {
                player
                    .update(cx, |p, _| p.duration())
                    .map_or(0.0, |d| d.as_secs_f64())
            });
            self.emit(emits);
            self.call(player, calls, cx);
            if self.events.borrow().progresses() {
                self.tick();
            }
        }

        /// Reports progress every `progressUpdateInterval` while playing.
        fn tick(&self) {
            if self.ticking.replace(true) {
                return;
            }
            self.schedule_tick();
        }

        fn schedule_tick(&self) {
            let interval = self
                .props
                .borrow()
                .as_ref()
                .map_or(250.0, |p| p.progress_interval_ms.max(16.0));
            let key = self.surface.upgrade().map(|s| s.key());
            let id = self.id;
            crate::rt::after_ms(interval as u64, move || {
                crate::rt::on_app(move |cx| {
                    let Some((surface, item)) = Self::find(key, id) else {
                        return;
                    };
                    let player = item.current();
                    let (Some(player), true) = (player, item.events.borrow().progresses()) else {
                        item.ticking.set(false);
                        return;
                    };
                    if let Ok(time) = player.update(cx, |p, _| p.position()) {
                        item.time.set(time.as_secs_f64());
                    }
                    let (time, duration) = (item.time.get(), item.duration.get());
                    item.emitter.emit(EventKind::Progress, || {
                        EventPayload::Map(vec![
                            ("currentTime", PayloadValue::Number(time)),
                            ("duration", PayloadValue::Number(duration)),
                        ])
                    });
                    if item.controls() {
                        surface.invalidate();
                    }
                    item.schedule_tick();
                });
            });
        }

        fn controls(&self) -> bool {
            self.props.borrow().as_ref().is_some_and(|p| p.controls)
        }

        /// Plays or pauses (the control bar).
        fn toggle(&self) {
            let playing = self.events.borrow().playing;
            self.apply(if playing { Op::Pause } else { Op::Play });
        }

        /// What the view's drawing does to the video at `size`.
        fn placement(cx: &DrawCx<'_>, size: (f32, f32)) -> Placement {
            let bounds = cx.rect(0.0, 0.0, size.0, size.1);
            Placement {
                axis_aligned: cx.axis_aligned(),
                opacity: cx.opacity,
                filtered: cx.matrix.is_some(),
                clipped: cx.has_clip_polygon()
                    || cx.rounded.is_some()
                    || cx.clip.intersect(&bounds) != bounds,
            }
        }
    }

    /// The platform's notice as the item's.
    fn of_platform(notice: platform::Notice) -> Notice {
        match notice {
            platform::Notice::Ready => Notice::Ready,
            platform::Notice::Playing => Notice::Playing,
            platform::Notice::Paused => Notice::Paused,
            platform::Notice::Seeked => Notice::Seeked,
            platform::Notice::Ended => Notice::Ended,
            platform::Notice::Failed(error) => Notice::Failed(format!("{error:?}")),
            platform::Notice::Buffering(percent) => Notice::Buffering(percent),
            platform::Notice::Interrupted(state) => Notice::Interrupted(match state {
                platform::State::Idle | platform::State::Destroyed => Left::Lost,
                platform::State::Ready | platform::State::Paused => Left::Paused,
                // Playing still, or a prepare in flight that will make it
                // ready.
                platform::State::Playing | platform::State::Preparing => Left::Kept,
            }),
            platform::Notice::Frame => Notice::Frame,
        }
    }

    impl Hosted for VideoItem {
        fn draw(&self, cx: &mut DrawCx<'_>, size: (f32, f32)) {
            let (w, h) = size;
            let wanted = output(&Self::placement(cx, size));
            let current = self.player.borrow().as_ref().map(|o| o.output);
            if current.is_some_and(|current| reopen(current, wanted)) {
                // Its view turned, faded or was clipped: on as a texture,
                // where it was.
                let waited = self.gate.borrow_mut().reopen();
                let playing = self.events.borrow().playing
                    || (waited && self.props.borrow().as_ref().is_some_and(|p| !p.paused));
                self.player.borrow_mut().take();
                self.seek_to.set(Some(self.time.get()));
                let mut again = self
                    .props
                    .borrow()
                    .as_ref()
                    .map(|p| ops(None, p))
                    .unwrap_or_default();
                again.retain(|op| !matches!(op, Op::Open(_) | Op::Play));
                if playing {
                    again.push(Op::Play);
                }
                *self.pending.borrow_mut() = again;
            }
            if self.player.borrow().is_none() && self.uri.borrow().is_some() && !cx.culled() {
                self.start(wanted, cx);
            }
            cx.push_clip_rect(0.0, 0.0, w, h);
            let player = self.player.borrow().as_ref().map(|o| o.player.clone());
            if let (Some(player), false) = (player, cx.culled()) {
                let (bounds, corners) =
                    cx.clip_rounded(cx.rect(0.0, 0.0, w, h), gpui::Corners::default());
                cx.paint(|window, app| {
                    player.update(app, |p, _| p.paint(bounds, corners, window));
                });
            }
            if self.controls() {
                let playing = self.events.borrow().playing;
                controls::draw_bar(
                    cx,
                    size,
                    playing,
                    self.time.get(),
                    self.duration.get(),
                    self.scrubbing.get(),
                );
            }
            cx.pop();
        }

        fn hit(&self, x: f32, y: f32, size: (f32, f32)) -> bool {
            // Only the control bar: presses elsewhere go to the view.
            self.controls() && x >= 0.0 && x < size.0 && y >= size.1 - BAR && y < size.1
        }

        fn pointer(&self, event: &ItemPointer) -> bool {
            let Some(surface) = self.surface.upgrade() else {
                return false;
            };
            let Some(width) = surface.view(self.id).map(|v| v.state().frame.size.width.0) else {
                return false;
            };
            let scrub = |x: f32| {
                if let Some(at) = controls::scrubbed(x, width, self.duration.get()) {
                    self.seek(at);
                }
            };
            match event.kind {
                PointerKind::Down if event.x < BUTTON => self.toggle(),
                PointerKind::Down => {
                    self.scrubbing.set(true);
                    scrub(event.x);
                }
                PointerKind::Move if self.scrubbing.get() => scrub(event.x),
                PointerKind::Up | PointerKind::Cancel => self.scrubbing.set(false),
                _ => return false,
            }
            surface.invalidate();
            true
        }

        fn cursor(&self, x: f32, _y: f32) -> Option<Cursor> {
            self.controls().then_some(if x < BUTTON {
                Cursor::Hand
            } else {
                Cursor::Arrow
            })
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    const PLAIN: Placement = Placement {
        axis_aligned: true,
        opacity: 1.0,
        filtered: false,
        clipped: false,
    };

    #[test]
    fn the_plane_for_a_video_drawn_as_laid_out_else_the_texture() {
        assert_eq!(output(&PLAIN), None);
        for placement in [
            Placement {
                axis_aligned: false,
                ..PLAIN
            },
            Placement {
                opacity: 0.5,
                ..PLAIN
            },
            Placement {
                filtered: true,
                ..PLAIN
            },
            Placement {
                clipped: true,
                ..PLAIN
            },
        ] {
            assert_eq!(output(&placement), Some(Output::Texture), "{placement:?}");
        }
        // A video on the plane that needs the texture opens again; never back.
        assert!(reopen(Output::Plane, Some(Output::Texture)));
        assert!(!reopen(Output::Plane, None));
        assert!(!reopen(Output::Texture, None));
        assert!(!reopen(Output::Texture, Some(Output::Texture)));
    }

    fn props(source: Option<&str>) -> VideoProps {
        VideoProps {
            source: source.map(Into::into),
            ..VideoProps::default()
        }
    }

    #[test]
    fn a_new_source_opens_with_the_props_and_plays_unless_paused() {
        let p = VideoProps {
            muted: true,
            volume: 0.5,
            repeat: true,
            ..props(Some("asset:/clip.mp4"))
        };
        assert_eq!(
            ops(None, &p),
            vec![
                Op::Open("asset:/clip.mp4".into()),
                Op::Muted(true),
                Op::Volume(0.5),
                Op::Looping(true),
                Op::Play,
            ]
        );
        let paused = VideoProps {
            paused: true,
            ..p.clone()
        };
        assert!(!ops(None, &paused).contains(&Op::Play));
        // Another source opens again; none closes.
        let other = props(Some("https://example.com/b.mp4"));
        assert_eq!(
            ops(Some(&p), &other)[0],
            Op::Open("https://example.com/b.mp4".into())
        );
        assert_eq!(ops(Some(&p), &props(None)), vec![Op::Close]);
        assert_eq!(ops(None, &props(None)), Vec::new());
    }

    #[test]
    fn changed_props_are_the_players_calls() {
        let p = props(Some("a.mp4"));
        assert_eq!(ops(Some(&p), &p), Vec::new());
        let changed = VideoProps {
            paused: true,
            muted: true,
            volume: 0.25,
            repeat: true,
            // No public API: nothing to call.
            rate: 2.0,
            ..p.clone()
        };
        assert_eq!(
            ops(Some(&p), &changed),
            vec![
                Op::Pause,
                Op::Muted(true),
                Op::Volume(0.25),
                Op::Looping(true)
            ]
        );
        // Volumes past 1 are 1, as the player takes them.
        let loud = VideoProps {
            volume: 3.0,
            ..p.clone()
        };
        assert_eq!(ops(Some(&p), &loud), Vec::new());
        assert!(ops(None, &loud).contains(&Op::Volume(1.0)));
        let playing = VideoProps {
            paused: false,
            ..changed.clone()
        };
        assert_eq!(ops(Some(&changed), &playing), vec![Op::Play]);
        let nan = VideoProps {
            volume: f32::NAN,
            ..p.clone()
        };
        assert_eq!(ops(Some(&p), &nan), Vec::new());
    }

    #[test]
    fn the_players_notices_are_nshs_events() {
        let mut events = Events::default();
        let ready = events.notice(Notice::Ready, "a.mp4", || 12.5);
        assert_eq!(ready, vec![Emit::Load(12.5), Emit::Redraw]);
        // Once per source.
        assert_eq!(events.notice(Notice::Ready, "a.mp4", || 1.0), Vec::new());
        assert!(!events.progresses());
        events.notice(Notice::Playing, "a.mp4", || 0.0);
        assert!(events.playing && events.progresses());
        assert_eq!(
            events.notice(Notice::Paused, "a.mp4", || 0.0),
            vec![Emit::Redraw]
        );
        assert!(!events.playing && !events.progresses());
        for notice in [Notice::Seeked, Notice::Frame] {
            assert_eq!(events.notice(notice, "a.mp4", || 0.0), vec![Emit::Redraw]);
        }
        events.notice(Notice::Playing, "a.mp4", || 0.0);
        assert_eq!(
            events.notice(Notice::Buffering(30), "a.mp4", || 0.0),
            vec![Emit::Buffer(true)]
        );
        assert_eq!(
            events.notice(Notice::Buffering(60), "a.mp4", || 0.0),
            Vec::new()
        );
        assert_eq!(
            events.notice(Notice::Buffering(100), "a.mp4", || 0.0),
            vec![Emit::Buffer(false)]
        );
        assert_eq!(
            events.notice(Notice::Ended, "a.mp4", || 0.0),
            vec![Emit::End, Emit::Redraw]
        );
        assert!(events.ended && !events.playing);
        // Interrupted by the system: what it left the player in.
        events.notice(Notice::Interrupted(Left::Paused), "a.mp4", || 0.0);
        assert!(!events.playing);
        let failed = events.notice(Notice::Failed("InvalidOperation".into()), "a.mp4", || 0.0);
        assert!(matches!(&failed[0], Emit::Error(m) if m.starts_with("a.mp4: ")));
        assert_eq!(
            events.notice(Notice::Failed("InvalidOperation".into()), "a.mp4", || 0.0),
            Vec::new()
        );
        assert!(!events.progresses());
    }

    /// A seek reaches the player once it is prepared (`Notice::Ready`), and
    /// playing waits behind it.
    mod seek_before_ready {
        use super::*;

        #[test]
        fn a_seek_before_ready_is_issued_once_on_ready() {
            let mut gate = Gate::default();
            assert_eq!(gate.seek(12.0), Vec::new());
            // The latest one.
            assert_eq!(gate.seek(30.5), Vec::new());
            assert_eq!(gate.ready(), vec![Call::Seek(30.5)]);
            assert_eq!(gate.ready(), Vec::new());
            // Prepared: at once.
            assert_eq!(gate.seek(4.0), vec![Call::Seek(4.0)]);
            assert_eq!(gate.op(Op::Pause), vec![Call::Op(Op::Pause)]);
            assert_eq!(gate.op(Op::Play), vec![Call::Op(Op::Play)]);
        }

        /// A video on the plane opened again as a texture: the new player
        /// is told the props, the time it was at and to play, and it plays
        /// from there once it is prepared.
        #[test]
        fn a_reopened_video_goes_on_where_it_was() {
            let mut gate = Gate::default();
            gate.ready();
            assert!(!gate.reopen());
            let mut calls = gate.seek(42.0);
            for op in [
                Op::Muted(true),
                Op::Volume(0.5),
                Op::Looping(false),
                Op::Play,
            ] {
                calls.extend(gate.op(op));
            }
            assert_eq!(
                calls,
                vec![
                    Call::Op(Op::Muted(true)),
                    Call::Op(Op::Volume(0.5)),
                    Call::Op(Op::Looping(false))
                ]
            );
            assert_eq!(gate.ready(), vec![Call::Seek(42.0), Call::Op(Op::Play)]);
            assert_eq!(gate.ready(), Vec::new());
        }

        #[test]
        fn paused_before_ready_seeks_without_playing() {
            let mut gate = Gate::default();
            gate.op(Op::Play);
            gate.seek(3.0);
            // `paused` came back before it was ready.
            assert_eq!(gate.op(Op::Pause), Vec::new());
            assert_eq!(gate.ready(), vec![Call::Seek(3.0)]);
            // Nothing waited.
            let mut gate = Gate::default();
            assert_eq!(gate.ready(), Vec::new());
            let mut gate = Gate::default();
            gate.op(Op::Play);
            assert_eq!(gate.ready(), vec![Call::Op(Op::Play)]);
        }
    }

    /// An interrupted player comes back once the app is active, and the
    /// app hears it stalled meanwhile.
    mod interruption {
        use super::*;

        fn playing_props() -> VideoProps {
            VideoProps {
                muted: true,
                volume: 0.25,
                repeat: true,
                ..props(Some("a.mp4"))
            }
        }

        fn ready() -> Gate {
            let mut gate = Gate::default();
            gate.ready();
            gate
        }

        #[test]
        fn unprepared_it_is_prepared_again_with_its_props_and_time() {
            let mut gate = ready();
            assert_eq!(
                gate.interrupted(Left::Lost, true, 61.5),
                vec![Call::Stalled(true)]
            );
            assert!(gate.waits());
            // Nothing reaches the player meanwhile.
            assert_eq!(gate.seek(1.0), Vec::new());
            assert_eq!(gate.seek(65.0), Vec::new());
            assert_eq!(gate.op(Op::Muted(false)), vec![Call::Op(Op::Muted(false))]);
            // The app is active.
            assert_eq!(
                gate.resume(Some(&playing_props())),
                vec![
                    Call::Prepare,
                    Call::Op(Op::Muted(true)),
                    Call::Op(Op::Volume(0.25)),
                    Call::Op(Op::Looping(true)),
                ]
            );
            assert!(!gate.waits());
            assert_eq!(gate.seek(66.0), Vec::new());
            assert_eq!(
                gate.ready(),
                vec![Call::Seek(66.0), Call::Op(Op::Play), Call::Stalled(false)]
            );
            assert_eq!(gate.resume(Some(&playing_props())), Vec::new());
        }

        #[test]
        fn unprepared_while_paused_it_comes_back_paused() {
            let mut gate = ready();
            gate.interrupted(Left::Lost, false, 8.0);
            assert_eq!(gate.resume(None), vec![Call::Prepare]);
            assert_eq!(gate.ready(), vec![Call::Seek(8.0), Call::Stalled(false)]);
            // Paused while it waited.
            let mut gate = ready();
            gate.interrupted(Left::Lost, true, 8.0);
            assert_eq!(gate.op(Op::Pause), Vec::new());
            gate.resume(None);
            assert_eq!(gate.ready(), vec![Call::Seek(8.0), Call::Stalled(false)]);
        }

        #[test]
        fn paused_by_the_system_it_plays_again_as_the_props_say() {
            let mut gate = ready();
            assert_eq!(
                gate.interrupted(Left::Paused, true, 20.0),
                vec![Call::Stalled(true)]
            );
            assert_eq!(
                gate.resume(Some(&playing_props())),
                vec![Call::Op(Op::Play), Call::Stalled(false)]
            );
            // Sought while it waited.
            let mut gate = ready();
            gate.interrupted(Left::Paused, true, 20.0);
            gate.seek(25.0);
            assert_eq!(
                gate.resume(None),
                vec![Call::Seek(25.0), Call::Op(Op::Play), Call::Stalled(false)]
            );
        }

        #[test]
        fn nothing_lost_nothing_to_do() {
            let mut gate = ready();
            // Paused as the props say, or still playing.
            assert_eq!(gate.interrupted(Left::Paused, false, 3.0), Vec::new());
            assert_eq!(gate.interrupted(Left::Kept, true, 3.0), Vec::new());
            assert!(!gate.waits());
            assert_eq!(gate.resume(None), Vec::new());
            assert_eq!(gate.seek(4.0), vec![Call::Seek(4.0)]);
        }

        /// Interrupted while it prepared: the seek and playing that waited
        /// wait for the app now.
        #[test]
        fn what_waited_for_ready_waits_for_the_app() {
            let mut gate = Gate::default();
            gate.seek(12.0);
            gate.op(Op::Play);
            assert_eq!(
                gate.interrupted(Left::Lost, false, 0.0),
                vec![Call::Stalled(true)]
            );
            // Interrupted again: once is enough.
            assert_eq!(gate.interrupted(Left::Lost, false, 0.0), Vec::new());
            assert_eq!(gate.resume(None), vec![Call::Prepare]);
            assert_eq!(
                gate.ready(),
                vec![Call::Seek(12.0), Call::Op(Op::Play), Call::Stalled(false)]
            );
            // A prepare from before came through while it waited: no second
            // prepare, but back to where it was.
            let mut gate = ready();
            gate.interrupted(Left::Lost, true, 30.0);
            assert_eq!(gate.ready(), Vec::new());
            assert_eq!(
                gate.resume(None),
                vec![Call::Seek(30.0), Call::Op(Op::Play), Call::Stalled(false)]
            );
        }

        /// Opened again as a texture while it waited: the new player goes
        /// on, and the stall ends once it is ready.
        #[test]
        fn reopened_while_waiting_the_stall_ends_on_ready() {
            let mut gate = ready();
            gate.interrupted(Left::Lost, true, 9.0);
            assert!(gate.reopen());
            assert!(!gate.waits());
            gate.seek(9.0);
            gate.op(Op::Play);
            assert_eq!(
                gate.ready(),
                vec![Call::Seek(9.0), Call::Op(Op::Play), Call::Stalled(false)]
            );
        }

        #[test]
        fn the_app_hears_the_stall_once_and_playing_stops() {
            let mut events = Events::default();
            events.notice(Notice::Ready, "a.mp4", || 10.0);
            events.notice(Notice::Playing, "a.mp4", || 0.0);
            assert_eq!(
                events.notice(Notice::Interrupted(Left::Kept), "a.mp4", || 0.0),
                vec![Emit::Redraw]
            );
            assert!(events.playing);
            events.notice(Notice::Interrupted(Left::Lost), "a.mp4", || 0.0);
            assert!(!events.playing && !events.progresses());
            assert_eq!(events.stalled(true), vec![Emit::Buffer(true)]);
            assert_eq!(events.stalled(true), Vec::new());
            // Ready again: no second onLoad.
            assert_eq!(events.notice(Notice::Ready, "a.mp4", || 10.0), Vec::new());
            assert_eq!(events.stalled(false), vec![Emit::Buffer(false)]);
            assert_eq!(events.stalled(false), Vec::new());
        }
    }

    #[test]
    fn events_carry_nshs_payloads() {
        let (kind, payload) = Emit::Load(f64::INFINITY).event().unwrap();
        assert_eq!(kind, EventKind::Load);
        assert_eq!(
            payload,
            EventPayload::Map(vec![
                ("duration", PayloadValue::Number(0.0)),
                ("width", PayloadValue::Number(0.0)),
                ("height", PayloadValue::Number(0.0)),
            ])
        );
        assert_eq!(
            Emit::Buffer(true).event().unwrap(),
            (
                EventKind::Buffer,
                EventPayload::Map(vec![("isBuffering", PayloadValue::Bool(true))])
            )
        );
        assert_eq!(Emit::End.event().unwrap().0, EventKind::End);
        assert_eq!(Emit::Error("x".into()).event().unwrap().0, EventKind::Error);
        assert_eq!(Emit::Redraw.event(), None);
    }
}
