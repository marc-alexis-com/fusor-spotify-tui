use crate::spotify::{Album, Artist, Playlist, Track};
use hypercmd::Key;
use std::{ops::Range, rc::Rc};

const EPISODE_PREFIX: &str = "spotify:episode:";

/// Something the main pane can list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Source {
    Search(String),
    Liked,
    Recent,
    Playlist(Rc<Playlist>),
    Album(Rc<Album>),
    Artist(Rc<Artist>),
    /// A podcast, which episodes name as their album.
    Show(Rc<Album>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Loading {
    Idle,
    Busy,
    Failed(String),
}

/// The tracks of a source. Every track URI is known up front; details load as rows come into view.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Listing {
    pub(crate) source: Source,
    pub(crate) uris: Rc<[String]>,
    pub(crate) tracks: Vec<Rc<Track>>,
    /// URIs sent for details, including the chunk in flight.
    pub(crate) requested: usize,
    /// URIs whose details arrived; unavailable tracks among them have no entry in `tracks`.
    pub(crate) loaded: usize,
    pub(crate) loading: Loading,
}

/// One rendered line of the main pane.
#[derive(Clone, PartialEq)]
pub(crate) struct Row {
    pub(crate) index: usize,
    pub(crate) marker: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) length: String,
    pub(crate) selected: bool,
    pub(crate) playing: bool,
    /// Spotify cannot play it here: the row is grayed out.
    pub(crate) unavailable: bool,
}

/// The visible slice of a list and its selected entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Window {
    pub(crate) selected: usize,
    top: usize,
    rows: usize,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            selected: 0,
            top: 0,
            rows: 1,
        }
    }
}

impl Window {
    pub(crate) fn resize(&mut self, rows: usize) {
        self.rows = rows.max(1);
        self.reveal();
    }

    /// Returns to a saved position, keeping the height the list has now.
    pub(crate) fn restore(&mut self, saved: Self) {
        let rows = self.rows;
        *self = saved;
        self.resize(rows);
    }

    pub(crate) fn select(&mut self, index: usize, length: usize) {
        self.selected = index.min(length.saturating_sub(1));
        self.reveal();
    }

    pub(crate) fn navigate(&mut self, key: Key, length: usize) {
        let target = match key {
            Key::Up => self.selected.saturating_sub(1),
            Key::Down => self.selected + 1,
            Key::PageUp => self.selected.saturating_sub(self.rows),
            Key::PageDown => self.selected + self.rows,
            Key::Home => 0,
            Key::End => length,
            _ => return,
        };
        self.select(target, length);
    }

    pub(crate) fn visible(&self, length: usize) -> Range<usize> {
        self.top.min(length)..(self.top + self.rows).min(length)
    }

    /// The rows of the scrollbar beside the window, out of `length` entries: the
    /// track above the thumb, the thumb, and the track below it. Empty when every
    /// entry fits.
    pub(crate) fn scrollbar(&self, length: usize) -> (usize, usize, usize) {
        if length <= self.rows {
            return (0, 0, 0);
        }
        let thumb = (self.rows * self.rows).div_ceil(length).clamp(1, self.rows);
        let room = self.rows - thumb;
        let scrollable = length - self.rows;
        let before = (self.top.min(scrollable) * room)
            .div_ceil(scrollable)
            .min(room);
        (before, thumb, room - before)
    }

    /// The first row past the window, which should be loaded before it scrolls into view.
    pub(crate) fn end(&self) -> usize {
        self.top + self.rows
    }

    fn reveal(&mut self) {
        if self.selected < self.top {
            self.top = self.selected;
        } else if self.selected >= self.top + self.rows {
            self.top = self.selected + 1 - self.rows;
        }
    }
}

impl Source {
    pub(crate) fn title(&self) -> String {
        match self {
            Self::Search(text) if text.is_empty() => "Search".into(),
            Self::Search(text) => format!("Search · “{text}”"),
            Self::Liked => "Liked Songs".into(),
            Self::Recent => "Recently played".into(),
            Self::Playlist(playlist) => playlist.name.clone(),
            Self::Album(album) => format!("Album · {}", album.name),
            Self::Artist(artist) => format!("Artist · {}", artist.name),
            Self::Show(show) => format!("Podcast · {}", show.name),
        }
    }

    pub(crate) fn empty_message(&self) -> &'static str {
        match self {
            Self::Search(text) if text.is_empty() => "Type what you want to hear and press Enter.",
            Self::Search(_) => "Nothing matched. Try other words.",
            Self::Liked => "Songs you like in Spotify appear here.",
            Self::Recent => "Nothing played recently.",
            Self::Playlist(_) | Self::Album(_) | Self::Artist(_) | Self::Show(_) => {
                "No playable tracks here."
            }
        }
    }
}

impl Listing {
    /// Idle, or stopped by a failed chunk; a list that never resolved has nothing to retry.
    pub(crate) fn can_load_more(&self) -> bool {
        match self.loading {
            Loading::Idle => true,
            Loading::Busy => false,
            Loading::Failed(_) => !self.uris.is_empty(),
        }
    }

    pub(crate) fn new(source: Source) -> Self {
        Self {
            source,
            uris: Rc::from([]),
            tracks: Vec::new(),
            requested: 0,
            loaded: 0,
            loading: Loading::Busy,
        }
    }

    /// Claims the next `size` URIs to load, unless `rows` tracks are loaded or none are left.
    pub(crate) fn next_chunk(&mut self, rows: usize, size: usize) -> Option<Vec<String>> {
        // Count loaded tracks, not requested URIs: unavailable tracks fill no row.
        if self.tracks.len() >= rows || self.requested >= self.uris.len() {
            self.loading = Loading::Idle;
            return None;
        }
        let start = self.requested;
        self.requested = (start + size).min(self.uris.len());
        self.loading = Loading::Busy;
        self.uris.get(start..self.requested).map(<[String]>::to_vec)
    }

    /// Adds the details of the chunk in flight.
    pub(crate) fn add_chunk(&mut self, tracks: impl IntoIterator<Item = Rc<Track>>) {
        self.tracks.extend(tracks);
        self.loaded = self.requested;
    }

    /// Forgets the chunk in flight, so that it is requested again.
    pub(crate) fn rewind(&mut self) {
        self.requested = self.loaded;
    }

    /// The tracks loaded plus those not loaded yet, leaving out the ones found unavailable.
    pub(crate) fn count(&self) -> usize {
        self.tracks.len() + (self.uris.len() - self.loaded)
    }

    /// Whether every entry is a podcast episode, whose columns are named after them.
    pub(crate) fn episodes_only(&self) -> bool {
        !self.uris.is_empty() && self.uris.iter().all(|uri| uri.starts_with(EPISODE_PREFIX))
    }

    /// Whether some entries are podcast episodes, which Spotify does not play from
    /// the context of a playlist.
    pub(crate) fn has_episodes(&self) -> bool {
        self.uris.iter().any(|uri| uri.starts_with(EPISODE_PREFIX))
    }

    /// Whether the source has nothing to play, once every track is known.
    pub(crate) fn is_empty(&self) -> bool {
        self.count() == 0 && self.loading == Loading::Idle
    }

    pub(crate) fn summary(&self) -> String {
        match &self.loading {
            Loading::Busy if self.tracks.is_empty() => "Loading…".into(),
            Loading::Failed(error) => error.clone(),
            _ => {
                let count = self.count();
                let noun = if self.episodes_only() {
                    "episode"
                } else {
                    "song"
                };
                let plural = if count == 1 { "" } else { "s" };
                format!("{count} {noun}{plural}")
            }
        }
    }

    pub(crate) fn rows(&self, window: Window, playing: Option<&str>) -> Vec<Row> {
        let visible = window.visible(self.tracks.len());
        self.tracks[visible.clone()]
            .iter()
            .zip(visible)
            .map(|(track, index)| {
                let playing = playing == Some(track.uri.as_str());
                Row {
                    index,
                    marker: if playing {
                        "♫".into()
                    } else {
                        (index + 1).to_string()
                    },
                    title: track.name.clone(),
                    // An episode has no artists: its show and release date take their place.
                    artist: match &track.released {
                        Some(_) => track.album.name.clone(),
                        None => track.artist_names(),
                    },
                    album: track
                        .released
                        .clone()
                        .unwrap_or_else(|| track.album.name.clone()),
                    length: duration(track.duration_ms),
                    selected: window.selected == index,
                    playing,
                    unavailable: !track.playable,
                }
            })
            .collect()
    }
}

/// Formats milliseconds as `m:ss`, or `h:mm:ss` for an hour or more.
pub(crate) fn duration(milliseconds: u64) -> String {
    let seconds = milliseconds / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::{Key, Listing, Source, Window, duration};
    use crate::spotify::{Album, Track};
    use std::rc::Rc;

    fn track(uri: &str) -> Rc<Track> {
        Rc::new(Track {
            uri: uri.into(),
            name: uri.into(),
            artists: Vec::new(),
            album: Album {
                uri: "spotify:album:a".into(),
                name: "Album".into(),
            },
            duration_ms: 0,
            released: None,
            playable: true,
        })
    }

    fn uris(count: usize) -> Rc<[String]> {
        (0..count)
            .map(|index| format!("spotify:track:{index}"))
            .collect()
    }

    #[test]
    fn short_chunks_keep_loading_until_the_rows_fill() {
        let mut listing = Listing::new(Source::Liked);
        listing.uris = uris(5);
        assert_eq!(listing.next_chunk(2, 2).map(|chunk| chunk.len()), Some(2));
        // Neither track of the first chunk is available.
        listing.add_chunk([]);
        assert_eq!(listing.next_chunk(2, 2).map(|chunk| chunk.len()), Some(2));
        listing.add_chunk([track("spotify:track:2"), track("spotify:track:3")]);
        assert_eq!(listing.next_chunk(2, 2), None);
        assert_eq!(listing.next_chunk(3, 2).map(|chunk| chunk.len()), Some(1));
        listing.add_chunk([]);
        assert_eq!(listing.next_chunk(3, 2), None);
        assert_eq!(listing.loading, super::Loading::Idle);
    }

    #[test]
    fn counts_leave_out_unavailable_tracks() {
        let mut listing = Listing::new(Source::Liked);
        listing.uris = uris(3);
        assert_eq!(listing.next_chunk(1, 2).map(|chunk| chunk.len()), Some(2));
        assert_eq!(listing.count(), 3);
        listing.add_chunk([track("spotify:track:1")]);
        assert_eq!((listing.count(), listing.summary()), (2, "2 songs".into()));
        assert_eq!(listing.next_chunk(1, 2), None);
        assert!(!listing.is_empty());
    }

    #[test]
    fn episodes_show_their_show_and_release_date() {
        let mut listing = Listing::new(Source::Liked);
        let mut episode = (*track("spotify:episode:e")).clone();
        episode.album.name = "The Show".into();
        episode.released = Some("2026-07-19".into());
        listing.uris = ["spotify:episode:e".to_owned()].into();
        listing.tracks = vec![Rc::new(episode), track("spotify:track:t")];
        let mut window = Window::default();
        window.resize(2);
        let rows = listing.rows(window, None);
        assert_eq!(
            (rows[0].artist.as_str(), rows[0].album.as_str()),
            ("The Show", "2026-07-19")
        );
        assert_eq!(rows[1].album, "Album");
        assert!(listing.episodes_only());
    }

    #[test]
    fn podcast_listings_count_episodes() {
        let mut listing = Listing::new(Source::Liked);
        listing.loading = super::Loading::Idle;
        listing.uris = ["spotify:episode:a", "spotify:episode:b"]
            .map(String::from)
            .into();
        assert_eq!(listing.summary(), "2 episodes");
        assert!(listing.has_episodes());
        listing.uris = ["spotify:episode:a", "spotify:track:b"]
            .map(String::from)
            .into();
        assert_eq!(listing.summary(), "2 songs");
        listing.uris = ["spotify:track:b"].map(String::from).into();
        assert_eq!(listing.summary(), "1 song");
        assert!(!listing.has_episodes());
    }

    #[test]
    fn unavailable_tracks_leave_an_empty_listing() {
        let mut listing = Listing::new(Source::Liked);
        listing.uris = uris(2);
        assert!(listing.next_chunk(1, 2).is_some());
        listing.add_chunk([]);
        assert_eq!(listing.next_chunk(1, 2), None);
        assert!(listing.is_empty());
    }

    #[test]
    fn rewound_chunks_do_not_repeat_loaded_tracks() {
        let mut listing = Listing::new(Source::Liked);
        listing.uris = uris(4);
        listing.requested = 2;
        // The second URI is unavailable and has no details.
        listing.add_chunk([track("spotify:track:0")]);
        listing.requested = 4;
        listing.rewind();
        assert_eq!(listing.requested, 2);
    }

    #[test]
    fn durations_use_minutes_and_hours() {
        assert_eq!(duration(0), "0:00");
        assert_eq!(duration(65_400), "1:05");
        assert_eq!(duration(3_723_000), "1:02:03");
    }

    #[test]
    fn window_keeps_the_selection_visible() {
        let mut window = Window::default();
        window.resize(3);
        for _ in 0..4 {
            window.navigate(Key::Down, 10);
        }
        assert_eq!(window.selected, 4);
        assert_eq!(window.visible(10), 2..5);
        window.navigate(Key::End, 10);
        assert_eq!(window.visible(10), 7..10);
        window.navigate(Key::PageUp, 10);
        assert_eq!((window.selected, window.visible(10)), (6, 6..9));
        window.navigate(Key::Home, 10);
        assert_eq!(window.visible(10), 0..3);
    }

    #[test]
    fn window_clamps_to_short_lists() {
        let mut window = Window::default();
        window.resize(5);
        window.navigate(Key::PageDown, 2);
        assert_eq!((window.selected, window.visible(2)), (1, 0..2));
        window.select(9, 0);
        assert_eq!((window.selected, window.visible(0)), (0, 0..0));
    }

    #[test]
    fn the_scrollbar_follows_the_window() {
        let mut window = Window::default();
        window.resize(10);
        assert_eq!(window.scrollbar(10), (0, 0, 0));
        assert_eq!(window.scrollbar(100), (0, 1, 9));
        window.navigate(Key::End, 100);
        assert_eq!(window.scrollbar(100), (9, 1, 0));
        window.navigate(Key::Home, 20);
        assert_eq!(window.scrollbar(20), (0, 5, 5));
        // Halfway down, the thumb is halfway down.
        window.select(14, 20);
        assert_eq!(window.scrollbar(20), (3, 5, 2));
        window.select(19, 20);
        assert_eq!(window.scrollbar(20), (5, 5, 0));
        window.select(0, 20);
        window.select(9, 20);
        // Still at the top: the selection only reached the last visible row.
        assert_eq!(window.scrollbar(20).0, 0);
        window.select(10, 20);
        assert_eq!(window.scrollbar(20).0, 1);
    }

    #[test]
    fn restored_windows_keep_the_current_height() {
        let mut saved = Window::default();
        saved.resize(39);
        saved.select(24, 100);
        let mut window = Window::default();
        window.resize(19);
        window.restore(saved);
        assert_eq!((window.selected, window.visible(100)), (24, 6..25));
    }
}
