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
//!   `rate` has no public native API on Tizen 8.0: it stays 1.
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
    /// The system interrupted it (another app took the decoder); whether
    /// it plays still.
    Interrupted {
        playing: bool,
    },
    /// A new frame for the texture.
    Frame,
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
            Notice::Interrupted { playing } => {
                self.playing = playing;
                vec![Emit::Redraw]
            }
        }
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
    use nsh_core::{EventKind, EventPayload, NodeId, PayloadValue, VideoProps};

    use super::{Emit, Events, Notice, Op, Output, Placement, ops, output, reopen};
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

        /// Tells the player `op`, or keeps it for when it opens.
        fn apply(&self, op: Op) {
            let Some(player) = self.player.borrow().as_ref().map(|o| o.player.clone()) else {
                self.pending.borrow_mut().push(op);
                return;
            };
            let plays = matches!(op, Op::Play);
            crate::rt::on_app(move |cx| {
                let result = player.update(cx, |p, cx| match op {
                    Op::Play => p.play(cx),
                    Op::Pause => p.pause(cx),
                    Op::Muted(muted) => p.set_muted(muted),
                    Op::Volume(volume) => p.set_volume(volume),
                    Op::Looping(looping) => p.set_looping(looping),
                    Op::Open(_) | Op::Close => Ok(()),
                });
                if let Err(err) = result {
                    log::debug!("video: {err}");
                }
            });
            if plays {
                self.tick();
            }
        }

        /// `seek(seconds)`.
        pub(crate) fn seek(&self, seconds: f64) {
            let seconds = finite(seconds).max(0.0);
            self.time.set(seconds);
            let Some(player) = self.player.borrow().as_ref().map(|o| o.player.clone()) else {
                self.seek_to.set(Some(seconds));
                return;
            };
            crate::rt::on_app(move |cx| {
                let at = Duration::from_secs_f64(seconds);
                if let Err(err) = player.update(cx, |p, cx| p.seek(at, cx)) {
                    log::debug!("video: seek: {err}");
                }
            });
            if let Some(surface) = self.surface.upgrade() {
                surface.invalidate();
            }
        }

        /// The video's view was deleted (or its source went): the player
        /// goes.
        pub(crate) fn close(&self) {
            self.player.borrow_mut().take();
            self.uri.borrow_mut().take();
            self.pending.borrow_mut().clear();
            self.seek_to.set(None);
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
                        let Some(surface) = key.and_then(Surface::by_key) else {
                            return;
                        };
                        let item = match surface.native.hosts.borrow().get(&id) {
                            Some(Host::Video(item)) => item.clone(),
                            _ => return,
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
            // What waited for it: audio, looping, playing.
            let waiting = std::mem::take(&mut *self.pending.borrow_mut());
            let seek = self.seek_to.take();
            let result = player.update(cx.app, |p, cx| {
                for op in waiting {
                    let _ = match op {
                        Op::Play => p.play(cx),
                        Op::Pause => p.pause(cx),
                        Op::Muted(muted) => p.set_muted(muted),
                        Op::Volume(volume) => p.set_volume(volume),
                        Op::Looping(looping) => p.set_looping(looping),
                        Op::Open(_) | Op::Close => Ok(()),
                    };
                }
                match seek {
                    Some(at) => p.seek(Duration::from_secs_f64(at), cx),
                    None => Ok(()),
                }
            });
            if let Err(err) = result {
                log::debug!("video: {err}");
            }
        }

        /// What the player reported.
        fn notice(
            &self,
            surface: &Rc<Surface>,
            player: &Entity<VideoPlayer>,
            notice: Notice,
            cx: &mut gpui::App,
        ) {
            let source = self.uri.borrow().clone().unwrap_or_default();
            let emits = self.events.borrow_mut().notice(notice, &source, || {
                player
                    .update(cx, |p, _| p.duration())
                    .map_or(0.0, |d| d.as_secs_f64())
            });
            for emit in emits {
                if let Emit::Load(duration) = emit {
                    self.duration.set(finite(duration));
                }
                match emit.event() {
                    Some((kind, payload)) => self.emitter.emit(kind, || payload),
                    None => surface.invalidate(),
                }
            }
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
                    let Some(surface) = key.and_then(Surface::by_key) else {
                        return;
                    };
                    let item = match surface.native.hosts.borrow().get(&id) {
                        Some(Host::Video(item)) => item.clone(),
                        _ => return,
                    };
                    let player = item.player.borrow().as_ref().map(|o| o.player.clone());
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
            platform::Notice::Interrupted(state) => Notice::Interrupted {
                playing: state == platform::State::Playing,
            },
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
                let playing = self.events.borrow().playing;
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
        events.notice(Notice::Interrupted { playing: false }, "a.mp4", || 0.0);
        assert!(!events.playing);
        let failed = events.notice(Notice::Failed("InvalidOperation".into()), "a.mp4", || 0.0);
        assert!(matches!(&failed[0], Emit::Error(m) if m.starts_with("a.mp4: ")));
        assert_eq!(
            events.notice(Notice::Failed("InvalidOperation".into()), "a.mp4", || 0.0),
            Vec::new()
        );
        assert!(!events.progresses());
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
