use super::{Jukebox, listing::duration};
use crate::{
    cover::{self, Slot},
    icons,
    spotify::{
        Album, Command, DEVICE_NAME, Error as SpotifyError, INITIAL_VOLUME, Mode, Repeat, Speaker,
        Track,
    },
};
use hypercmd::{Error, Event, EventPayload};
use librespot_playback::player::{PlayerEvent, PlayerEventChannel};
use std::{
    rc::Rc,
    time::{Duration, Instant},
};

const TICK: Duration = Duration::from_millis(250);
pub(super) const SEEK_STEP: Duration = Duration::from_secs(10);
/// One step of the volume keys: 5% of the speaker's range.
const VOLUME_STEP: u16 = u16::MAX / 20;
/// Cells of the volume bar.
const VOLUME_CELLS: u32 = 12;
const PERCENT: u32 = 100;
/// Shades of the progress bar's edge cell between gray and green.
const PROGRESS_STEPS: usize = 8;
/// The player's border fades from green to gray in eighths when music pauses,
/// one eighth every two frames, and back when it plays.
pub(super) const FADE_LEVELS: u8 = 8;
const FRAMES_PER_FADE_LEVEL: usize = 2;
/// The progress bar moves at 60 frames per second while music plays.
const FRAME: Duration = Duration::from_millis(16);

/// What the speaker reports it is playing.
#[derive(Clone, PartialEq)]
pub(super) struct NowPlaying {
    uri: Option<String>,
    /// The track librespot was asked to play, which `uri` names differently when
    /// Spotify relinks it to a playable copy.
    requested: Option<String>,
    /// The track librespot is loading, which only becomes `uri` once it starts.
    loading: Option<String>,
    track: Option<Rc<Track>>,
    playing: bool,
    /// librespot ignores Resume once stopped, after the end of a list or `p` on its first track.
    stopped: bool,
    position: Duration,
    reported: Instant,
    volume: u16,
    shuffle: bool,
    repeat: Repeat,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum VolumeStep {
    Up,
    Down,
}

impl Default for NowPlaying {
    fn default() -> Self {
        Self {
            uri: None,
            requested: None,
            loading: None,
            track: None,
            playing: false,
            stopped: false,
            position: Duration::ZERO,
            reported: Instant::now(),
            volume: INITIAL_VOLUME,
            shuffle: false,
            repeat: Repeat::Off,
        }
    }
}

impl NowPlaying {
    fn report(&mut self, position_ms: u32, playing: bool) {
        self.position = Duration::from_millis(position_ms.into());
        self.reported = Instant::now();
        self.playing = playing;
        self.stopped = false;
    }

    /// librespot rewinds a stopped track to its start; the last report may be
    /// anywhere in it, such as where it last resumed.
    fn stop(&mut self) {
        self.report(0, false);
        self.stopped = true;
    }

    /// librespot turns repeat-one off on a skip without a RepeatChanged event;
    /// repeating the list is left as it was.
    fn skip(&mut self) {
        if self.repeat == Repeat::Track {
            self.repeat = Repeat::Context;
        }
    }

    /// Switches to `uri`; returns whether its details are already known, as
    /// when repeat-one starts the same track again.
    fn start(&mut self, uri: &str) -> bool {
        self.loading = None;
        self.stopped = false;
        if self.uri.as_deref() == Some(uri) && self.track.is_some() {
            return true;
        }
        self.uri = Some(uri.to_owned());
        self.requested = None;
        self.track = None;
        false
    }

    /// The playing track as lists name it.
    fn listed_uri(&self) -> Option<String> {
        self.requested.clone().or_else(|| self.uri.clone())
    }

    /// librespot also reports the next track when preloading it fails, while the
    /// current one plays on; it skips that one when it gets there.
    fn unavailable(&self, uri: Option<&str>) -> &'static str {
        if uri.is_some() && uri == self.loading.as_deref() {
            "Spotify can't play this track here; skipping it."
        } else {
            "Spotify can't play the next track here; it will be skipped."
        }
    }

    /// Adds the details of `uri`, unless another track started meanwhile.
    fn describe(&mut self, uri: &str, track: Track) {
        if self.uri.as_deref() == Some(uri) {
            self.track = Some(Rc::new(track));
        }
    }

    fn elapsed(&self) -> Duration {
        let since = if self.playing {
            self.reported.elapsed()
        } else {
            Duration::ZERO
        };
        let elapsed = self.position + since;
        match &self.track {
            Some(track) => elapsed.min(Duration::from_millis(track.duration_ms)),
            None => elapsed,
        }
    }
}

impl Jukebox {
    pub(super) fn start_clock(self: &Rc<Self>) -> Result<(), Error> {
        self.services.spawn(&self.owner, self.clone().run_clock())?;
        self.services.spawn(&self.owner, self.clone().run_frames())
    }

    async fn run_clock(self: Rc<Self>) {
        // The clock stops when the app is disposed or out of timers.
        while let Ok(sleep) = self.services.sleep(TICK) {
            if sleep.await.is_err() {
                return;
            }
            self.clock.update(|clock| *clock += 1);
        }
    }

    /// Counts frames while music plays; paused, it checks four times a second.
    async fn run_frames(self: Rc<Self>) {
        let mut frames: usize = 0;
        loop {
            let playing = self.now.with_untracked(|now| now.playing);
            let target = if playing { 0 } else { FADE_LEVELS };
            let fading = self.border_fade.get_untracked() != target;
            let pause = if playing || fading { FRAME } else { TICK };
            let Ok(sleep) = self.services.sleep(pause) else {
                return;
            };
            if sleep.await.is_err() {
                return;
            }
            frames = frames.wrapping_add(1);
            if playing {
                self.frame.update(|frame| *frame += 1);
            }
            if fading && frames.is_multiple_of(FRAMES_PER_FADE_LEVEL) {
                self.border_fade
                    .update(|level| *level = step_toward(*level, target));
            }
        }
    }

    /// Whether the player's border shows `level` eighths of the way from green to gray.
    pub(super) fn border_fade(&self, level: u8) -> bool {
        self.border_fade.get() == level
    }

    pub(super) fn follow_player(self: &Rc<Self>, events: PlayerEventChannel) -> Result<(), Error> {
        self.services
            .spawn(&self.owner, self.clone().run_player(events))
    }

    async fn run_player(self: Rc<Self>, mut events: PlayerEventChannel) {
        while let Some(event) = events.recv().await {
            self.apply(event);
        }
        self.warn("The speaker stopped. Restart spt to play again.".into());
    }

    fn apply(self: &Rc<Self>, event: PlayerEvent) {
        match event {
            PlayerEvent::TrackChanged { audio_item } => {
                let covers = audio_item.covers.iter();
                let cover = cover::pick(covers.map(|image| (image.width, image.url.as_str())));
                let fallback = fallback(
                    audio_item.uri.clone(),
                    audio_item.name,
                    audio_item.duration_ms,
                );
                self.change_track(audio_item.uri, cover, fallback);
            }
            PlayerEvent::Playing {
                position_ms,
                track_id,
                ..
            } => self.now.update(|now| {
                now.report(position_ms, true);
                now.requested = track_id.to_uri().ok();
            }),
            PlayerEvent::Paused {
                position_ms,
                track_id,
                ..
            } => self.now.update(|now| {
                now.report(position_ms, false);
                now.requested = track_id.to_uri().ok();
            }),
            PlayerEvent::Seeked { position_ms, .. }
            | PlayerEvent::PositionCorrection { position_ms, .. }
            | PlayerEvent::PositionChanged { position_ms, .. } => {
                self.now.update(|now| now.report(position_ms, now.playing));
            }
            PlayerEvent::Stopped { .. } => self.now.update(NowPlaying::stop),
            PlayerEvent::VolumeChanged { volume } => self.now.update(|now| now.volume = volume),
            PlayerEvent::ShuffleChanged { shuffle } => self.now.update(|now| now.shuffle = shuffle),
            PlayerEvent::RepeatChanged { context, track } => self.now.update(|now| {
                now.repeat = match (context, track) {
                    (_, true) => Repeat::Track,
                    (true, false) => Repeat::Context,
                    (false, false) => Repeat::Off,
                };
            }),
            PlayerEvent::Loading { track_id, .. } => {
                self.now.update(|now| now.loading = track_id.to_uri().ok());
            }
            PlayerEvent::Unavailable { track_id, .. } => {
                let uri = track_id.to_uri().ok();
                let notice = self
                    .now
                    .with_untracked(|now| now.unavailable(uri.as_deref()));
                self.warn(notice.into());
            }
            _ => {}
        }
    }

    fn change_track(self: &Rc<Self>, uri: String, cover: Option<String>, fallback: Track) {
        let known = self.now.update(|now| now.start(&uri));
        self.change_cover(&uri, cover);
        if known {
            return;
        }
        let Some(catalog) = self.catalog() else {
            self.now.update(|now| now.describe(&uri, fallback));
            return;
        };
        let now = self.now.clone();
        let started = self.spawn(async move {
            // A failed or empty answer would otherwise leave "Loading…" for the whole track.
            let found = catalog.tracks(vec![uri.clone()]).await;
            let track = found
                .as_ref()
                .ok()
                .and_then(|tracks| tracks.first().cloned());
            now.update(|now| now.describe(&uri, track.unwrap_or(fallback)));
            found.map(drop)
        });
        if let Err(error) = started {
            self.warn(error.to_string());
        }
    }

    /// Shows the cover of `uri`; an album keeps its cover from one track to the next.
    fn change_cover(&self, uri: &str, url: Option<String>) {
        if !cover::supported() || self.cover.with_untracked(|shown| *shown == url) {
            return;
        }
        self.cover.set(None);
        let Some(url) = url else {
            // Nothing shows the old cover any more; the terminal can free it.
            if let Err(error) = cover::clear(Slot::Player) {
                self.warn(format!("Cannot read the album cover ({error})"));
            }
            return;
        };
        let (http, now, shown, uri) = (
            self.http.clone(),
            self.now.clone(),
            self.cover.clone(),
            uri.to_owned(),
        );
        let started = self.spawn(async move {
            let pixels = cover::fetch(http, url.clone(), Slot::Player).await?;
            // Another track may have started during the download.
            if now.with_untracked(|now| now.uri.as_deref() == Some(uri.as_str())) {
                cover::show(Slot::Player, &pixels)
                    .map_err(|error| SpotifyError::Cover(error.to_string()))?;
                shown.set(Some(url));
            }
            Ok(())
        });
        if let Err(error) = started {
            self.warn(error.to_string());
        }
    }

    pub(super) fn with_speaker(
        &self,
        action: impl FnOnce(&Speaker) -> Result<(), crate::spotify::Error>,
    ) {
        let result = match self.speaker.borrow().as_ref() {
            Some(speaker) => action(speaker),
            None => return,
        };
        if let Err(error) = result {
            self.warn(error.to_string());
        }
    }

    pub(super) fn command(&self, command: Command) {
        self.with_speaker(|speaker| {
            speaker.command(command)?;
            if command == Command::Next {
                self.now.update(NowPlaying::skip);
            }
            Ok(())
        });
    }

    pub(super) fn toggle_play(&self) -> Result<(), Error> {
        let (started, playing) = self
            .now
            .with_untracked(|now| (now.uri.is_some() && !now.stopped, now.playing));
        if !started {
            return self.play_selected();
        }
        self.command(if playing {
            Command::Pause
        } else {
            Command::Resume
        });
        Ok(())
    }

    pub(super) fn seek_forward(&self) {
        // librespot ignores a seek past the end, so the last seconds would not skip.
        let target = self.now.with_untracked(|now| {
            let target = now.elapsed() + SEEK_STEP;
            match &now.track {
                Some(track) => target.min(Duration::from_millis(track.duration_ms)),
                None => target,
            }
        });
        self.seek(target);
    }

    pub(super) fn seek_back(&self) {
        let target = self
            .now
            .with_untracked(|now| now.elapsed().saturating_sub(SEEK_STEP));
        self.seek(target);
    }

    fn seek(&self, target: Duration) {
        // Positions past u32 milliseconds (49 days) clamp to the end of the track.
        let position = target.as_millis().try_into().unwrap_or(u32::MAX);
        self.command(Command::Seek(position));
    }

    pub(super) fn change_volume(&self, step: VolumeStep) {
        let volume = self.now.with_untracked(|now| match step {
            VolumeStep::Up => now.volume.saturating_add(VOLUME_STEP),
            VolumeStep::Down => now.volume.saturating_sub(VOLUME_STEP),
        });
        self.command(Command::Volume(volume));
    }

    pub(super) fn toggle_shuffle(&self) {
        let shuffle = self.now.with_untracked(|now| !now.shuffle);
        self.command(Command::Shuffle(shuffle));
    }

    pub(super) fn cycle_repeat(&self) {
        let repeat = self.now.with_untracked(|now| now.repeat.next());
        self.command(Command::Repeat(repeat));
    }

    pub(super) fn mode(&self) -> Mode {
        self.now.with_untracked(|now| Mode {
            shuffle: now.shuffle,
            repeat: now.repeat,
        })
    }

    pub(super) fn playing_uri(&self) -> Option<String> {
        self.now.with(NowPlaying::listed_uri)
    }
}

/// Text and state shown in the now-playing bar.
impl Jukebox {
    pub(super) fn is_playing(&self) -> bool {
        self.now.with(|now| now.playing)
    }

    pub(super) fn cover_shown(&self) -> bool {
        cover::supported()
    }

    /// One row of the cover, or blank cells until the terminal has the image.
    pub(super) fn cover_row(&self, row: usize) -> String {
        if self.cover.with(Option::is_some) {
            cover::row(Slot::Player, row)
        } else {
            " ".repeat(Slot::Player.columns())
        }
    }

    pub(super) fn track_title(&self) -> String {
        self.now.with(|now| match (&now.track, &now.uri) {
            (Some(track), _) => track.name.clone(),
            (None, Some(_)) => "Loading…".into(),
            (None, None) => "Nothing playing".into(),
        })
    }

    pub(super) fn track_details(&self) -> String {
        self.now.with(|now| match &now.track {
            Some(track) => details(track),
            None => format!("Choose a track and press Enter · plays here as “{DEVICE_NAME}”"),
        })
    }

    fn elapsed(&self) -> Duration {
        self.clock.get();
        self.now.with(NowPlaying::elapsed)
    }

    fn length(&self) -> Duration {
        self.now.with(|now| {
            now.track.as_ref().map_or(Duration::ZERO, |track| {
                Duration::from_millis(track.duration_ms)
            })
        })
    }

    pub(super) fn elapsed_label(&self) -> String {
        label(self.elapsed())
    }

    pub(super) fn length_label(&self) -> String {
        label(self.length())
    }

    /// The progress bar at this frame.
    fn progress(&self) -> Progress {
        self.frame.get();
        let length = self.length();
        let fraction = if length.is_zero() {
            0.0
        } else {
            self.elapsed().as_secs_f64() / length.as_secs_f64()
        };
        Progress::new(fraction, self.bar_width.get())
    }

    pub(super) fn progress_played(&self) -> String {
        "━".repeat(self.progress().played)
    }

    pub(super) fn progress_edge(&self) -> &'static str {
        if self.progress().edge.is_some() {
            "━"
        } else {
            ""
        }
    }

    /// Whether the cell between played and left shows `level` eighths of green.
    pub(super) fn edge_level(&self, level: u8) -> bool {
        self.progress().edge == Some(level)
    }

    pub(super) fn progress_left(&self) -> String {
        "━".repeat(self.progress().left)
    }

    pub(super) fn measure_bar(&self, event: &Event) {
        if let EventPayload::Resize { width, .. } = event.payload {
            self.bar_width.set(usize::from(width));
        }
    }

    /// The volume in percent, rounded: one step down from full volume is 95%, not 94%.
    fn percent(&self) -> u32 {
        let full = u32::from(u16::MAX);
        self.now
            .with(|now| (u32::from(now.volume) * PERCENT + full / 2) / full)
    }

    /// The Nerd Font icon of the volume's level, silent then three equal thirds.
    pub(super) fn volume_icon(&self) -> String {
        let level = icons::get().volume[self.percent().div_ceil(PERCENT / 3).min(3) as usize];
        if level.is_empty() {
            String::new()
        } else {
            format!("{level} ")
        }
    }

    /// The volume as a bar like the progress bar: its loud part, then the rest.
    pub(super) fn volume_bar(&self) -> (String, String) {
        volume_bar(self.percent())
    }

    pub(super) fn shuffle_on(&self) -> bool {
        self.now.with(|now| now.shuffle)
    }

    pub(super) fn repeat_mode(&self) -> Repeat {
        self.now.with(|now| now.repeat)
    }

    pub(super) fn repeat_label(&self) -> &'static str {
        match self.repeat_mode() {
            Repeat::Off | Repeat::Context => icons::get().repeat,
            Repeat::Track => icons::get().repeat_one,
        }
    }

    pub(super) fn play_label(&self) -> &'static str {
        if self.is_playing() {
            icons::get().pause
        } else {
            icons::get().play
        }
    }
}

/// The cells of the progress bar. The one between the played and left parts
/// fades from gray to green in eighths, so the bar moves smoother than a cell.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Progress {
    played: usize,
    /// Eighths of green in the edge cell, from 0 to 7.
    edge: Option<u8>,
    left: usize,
}

impl Progress {
    fn new(fraction: f64, width: usize) -> Self {
        let eighths = (fraction.clamp(0.0, 1.0) * (width * PROGRESS_STEPS) as f64).round() as usize;
        let played = (eighths / PROGRESS_STEPS).min(width);
        let edge = (played < width).then_some((eighths % PROGRESS_STEPS) as u8);
        Self {
            played,
            edge,
            left: width - played - usize::from(edge.is_some()),
        }
    }
}

/// What the player knows of a track, for when the catalog cannot describe it.
fn fallback(uri: String, name: String, duration_ms: u32) -> Track {
    Track {
        uri,
        name,
        artists: Vec::new(),
        album: Album {
            uri: String::new(),
            name: String::new(),
        },
        duration_ms: u64::from(duration_ms),
        released: None,
        playable: true,
    }
}

/// One fade level closer to `target`.
fn step_toward(level: u8, target: u8) -> u8 {
    if level < target { level + 1 } else { level - 1 }
}

/// A bar of [`VOLUME_CELLS`] cells, green up to `percent`.
fn volume_bar(percent: u32) -> (String, String) {
    let loud = (percent * VOLUME_CELLS).div_ceil(PERCENT).min(VOLUME_CELLS) as usize;
    ("━".repeat(loud), "━".repeat(VOLUME_CELLS as usize - loud))
}

/// The player alone names neither the artists nor the album.
fn details(track: &Track) -> String {
    [track.artist_names(), track.album.name.clone()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

fn label(length: Duration) -> String {
    // Saturates only past 584 million years.
    duration(length.as_millis().try_into().unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use super::{Album, NowPlaying, Repeat, Track, details, volume_bar};
    use crate::spotify::Artist;
    use std::time::Duration;

    #[test]
    fn the_progress_bar_moves_in_eighths_of_a_cell() {
        let progress = |fraction| super::Progress::new(fraction, 10);
        let parts = |fraction| {
            let bar = progress(fraction);
            (bar.played, bar.edge, bar.left)
        };
        assert_eq!(parts(0.0), (0, Some(0), 9));
        assert_eq!(parts(0.05), (0, Some(4), 9));
        assert_eq!(parts(0.1), (1, Some(0), 8));
        assert_eq!(parts(0.4875), (4, Some(7), 5));
        assert_eq!(parts(1.0), (10, None, 0));
        assert_eq!(parts(2.0), (10, None, 0));
        assert_eq!(
            super::Progress::new(0.5, 0),
            super::Progress {
                played: 0,
                edge: None,
                left: 0,
            }
        );
    }

    #[test]
    fn the_volume_bar_fills_up_to_the_volume() {
        let loud = |percent| volume_bar(percent).0.chars().count();
        assert_eq!(volume_bar(0), (String::new(), "━".repeat(12)));
        assert_eq!(loud(1), 1);
        assert_eq!(loud(50), 6);
        assert_eq!(volume_bar(100), ("━".repeat(12), String::new()));
    }

    #[test]
    fn skipping_ends_repeat_one_only() {
        let mut now = NowPlaying::default();
        for (before, after) in [
            (Repeat::Track, Repeat::Context),
            (Repeat::Context, Repeat::Context),
            (Repeat::Off, Repeat::Off),
        ] {
            now.repeat = before;
            now.skip();
            assert_eq!(now.repeat, after);
        }
    }

    #[test]
    fn only_a_loading_track_is_skipped_now() {
        let mut now = NowPlaying {
            uri: Some("spotify:track:playing".into()),
            loading: Some("spotify:track:loading".into()),
            ..NowPlaying::default()
        };
        assert!(
            now.unavailable(Some("spotify:track:loading"))
                .contains("this track")
        );
        assert!(
            now.unavailable(Some("spotify:track:next"))
                .contains("next track")
        );
        now.loading = None;
        assert!(now.unavailable(None).contains("next track"));
    }

    #[test]
    fn restarting_a_track_keeps_its_details() {
        let track = Track {
            uri: "spotify:track:one".into(),
            name: "One".into(),
            artists: Vec::new(),
            album: Album {
                uri: "spotify:album:one".into(),
                name: "One".into(),
            },
            duration_ms: 1_000,
            released: None,
            playable: true,
        };
        let mut now = NowPlaying::default();
        assert!(!now.start("spotify:track:one"));
        now.describe("spotify:track:one", track);
        assert!(now.start("spotify:track:one"));
        assert!(now.track.is_some());
        assert!(!now.start("spotify:track:two"));
        assert!(now.track.is_none());
    }

    #[test]
    fn details_leave_out_what_is_unknown() {
        let mut track = Track {
            uri: "spotify:episode:one".into(),
            name: "One".into(),
            artists: Vec::new(),
            album: Album {
                uri: String::new(),
                name: String::new(),
            },
            duration_ms: 1_000,
            released: None,
            playable: true,
        };
        assert_eq!(details(&track), "");
        track.artists.push(Artist {
            uri: "spotify:artist:one".into(),
            name: "Band".into(),
        });
        track.album.name = "Record".into();
        assert_eq!(details(&track), "Band · Record");
    }

    #[test]
    fn a_relinked_track_keeps_its_listed_uri() {
        let mut now = NowPlaying::default();
        now.start("spotify:track:copy");
        assert_eq!(now.listed_uri().as_deref(), Some("spotify:track:copy"));
        now.requested = Some("spotify:track:listed".into());
        assert_eq!(now.listed_uri().as_deref(), Some("spotify:track:listed"));
        now.start("spotify:track:next");
        assert_eq!(now.listed_uri().as_deref(), Some("spotify:track:next"));
    }

    #[test]
    fn stopping_rewinds_to_the_start() {
        let mut now = NowPlaying::default();
        now.report(60_000, true);
        now.stop();
        assert_eq!(now.elapsed(), Duration::ZERO);
        assert!(now.stopped && !now.playing);
    }
}
