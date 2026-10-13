mod browse;
mod header;
mod keys;
mod library;
mod library_search;
mod listing;
mod playback;
mod results;
mod session;
mod view;

use crate::spotify::{self, Catalog, LibraryEntry, PlaylistDetails, Speaker};
use fusor::{FromInputs, OwnerHandle, Signal, signal};
use header::Header;
use hypercmd::{Error, Node, Services};
use listing::{Listing, Source, Window};
use playback::NowPlaying;
use results::Found;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    future::Future,
    rc::Rc,
};

/// How long a notice stays on screen, in clock ticks.
const NOTICE_TICKS: usize = 16;

pub(crate) struct Startup {
    pub(crate) http: reqwest::Client,
    pub(crate) refresh_token: Option<String>,
}

#[derive(Clone, PartialEq)]
pub(crate) enum Screen {
    Connecting,
    LoggedOut,
    Waiting(String),
    Failed(String),
    Library,
}

#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Info,
    Error,
}

#[derive(Clone, PartialEq)]
struct Notice {
    text: String,
    tone: Tone,
    shown: usize,
}

pub(crate) struct Jukebox {
    owner: OwnerHandle,
    services: Services,
    http: reqwest::Client,
    speaker: RefCell<Option<Speaker>>,
    catalog: RefCell<Option<Catalog>>,
    screen: Signal<Screen>,
    library: Signal<Vec<LibraryEntry>>,
    /// IDs of the library folders the sidebar shows open.
    open_folders: Signal<HashSet<String>>,
    listing: Signal<Listing>,
    window: Signal<Window>,
    history: RefCell<Vec<(Listing, Window)>>,
    generation: Cell<u64>,
    /// When `g` was pressed, while a second `g` would go to the top.
    pending_g: Cell<Option<std::time::Instant>>,
    /// Whether the selection follows the end of the list while the rest loads, after End or `G`.
    to_end: Cell<bool>,
    search: Signal<String>,
    /// The artists, albums, playlists and podcasts of the last search.
    found: Signal<Option<Found>>,
    /// The Liked Songs with their details, once a library search loaded them.
    liked: library_search::LikedCache,
    /// The search box whose result has the focus, which outlines it.
    focused_box: Signal<Option<&'static str>>,
    now: Signal<NowPlaying>,
    clock: Signal<usize>,
    /// Frames of the progress bar, which counts faster than the clock while music plays.
    frame: Signal<u64>,
    /// How far the player's border has faded from green to gray, in eighths.
    border_fade: Signal<u8>,
    bar_width: Signal<usize>,
    /// The URL of the cover the terminal shows, once it is there.
    cover: Signal<Option<String>>,
    /// The details of the playlist opened last, for the header above its tracks.
    playlist: Signal<Option<Rc<PlaylistDetails>>>,
    /// The URL of the playlist cover the terminal shows, once it is there.
    playlist_cover: Signal<Option<String>>,
    /// Playlist headers already loaded, by playlist URI.
    headers: RefCell<HashMap<String, Header>>,
    /// Playlist headers on their way.
    fetching_headers: RefCell<HashSet<String>>,
    /// Whether the tracks of a playlist wait for its header.
    header_pending: Signal<bool>,
    /// The playlist the sidebar rests on, whose header may load ahead of Enter.
    focused_playlist: RefCell<Option<String>>,
    help: Signal<bool>,
    /// Whether a shelf has the focus, which outlines the sidebar.
    shelves_focused: Signal<bool>,
    notice: Signal<Option<Notice>>,
    root: RefCell<Option<Node>>,
    saved_login: RefCell<Option<String>>,
}

impl FromInputs for Jukebox {
    type Inputs = Startup;
    type Error = Error;

    fn from_inputs(startup: Startup, owner: OwnerHandle) -> Result<Self, Error> {
        let screen = match startup.refresh_token {
            Some(_) => Screen::Connecting,
            None => Screen::LoggedOut,
        };
        Ok(Self {
            services: Services::from_owner(&owner)?,
            owner,
            http: startup.http,
            speaker: RefCell::new(None),
            catalog: RefCell::new(None),
            screen: signal(screen),
            library: signal(Vec::new()),
            open_folders: signal(HashSet::new()),
            listing: signal(Listing::new(Source::Liked)),
            window: signal(Window::default()),
            history: RefCell::new(Vec::new()),
            generation: Cell::new(0),
            pending_g: Cell::new(None),
            to_end: Cell::new(false),
            search: signal(String::new()),
            found: signal(None),
            liked: Rc::default(),
            focused_box: signal(None),
            now: signal(NowPlaying::default()),
            clock: signal(0),
            frame: signal(0),
            // Nothing plays yet, so the border starts gray.
            border_fade: signal(playback::FADE_LEVELS),
            bar_width: signal(0),
            cover: signal(None),
            playlist: signal(None),
            playlist_cover: signal(None),
            headers: RefCell::new(HashMap::new()),
            fetching_headers: RefCell::new(HashSet::new()),
            header_pending: signal(false),
            focused_playlist: RefCell::new(None),
            help: signal(false),
            shelves_focused: signal(false),
            notice: signal(None),
            root: RefCell::new(None),
            saved_login: RefCell::new(startup.refresh_token),
        })
    }
}

impl Jukebox {
    fn catalog(&self) -> Option<Catalog> {
        self.catalog.borrow().clone()
    }

    /// Runs Spotify work in the background and reports its failure as a notice.
    fn spawn(
        &self,
        task: impl Future<Output = Result<(), spotify::Error>> + 'static,
    ) -> Result<(), Error> {
        let notice = self.notice.clone();
        let clock = self.clock.clone();
        self.services.spawn(&self.owner, async move {
            if let Err(error) = task.await {
                notify(&notice, &clock, error.to_string(), Tone::Error);
            }
        })
    }

    fn inform(&self, text: impl Into<String>) {
        notify(&self.notice, &self.clock, text.into(), Tone::Info);
    }

    fn warn(&self, text: String) {
        notify(&self.notice, &self.clock, text, Tone::Error);
    }

    fn notice_text(&self) -> String {
        let now = self.clock.get();
        self.notice.with(|notice| {
            notice
                .as_ref()
                .filter(|notice| now.saturating_sub(notice.shown) < NOTICE_TICKS)
                .map_or_else(String::new, |notice| notice.text.clone())
        })
    }

    fn notice_is_error(&self) -> bool {
        self.notice.with(|notice| {
            notice
                .as_ref()
                .is_some_and(|notice| notice.tone == Tone::Error)
        })
    }

    fn focus(&self, id: &str) {
        if let Some(node) = self.root.borrow().as_ref().and_then(|root| root.find(id)) {
            node.request_focus();
        }
    }
}

fn notify(notice: &Signal<Option<Notice>>, clock: &Signal<usize>, text: String, tone: Tone) {
    notice.set(Some(Notice {
        text,
        tone,
        shown: clock.get_untracked(),
    }));
}
