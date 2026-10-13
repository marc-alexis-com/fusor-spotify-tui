use super::{
    Jukebox,
    library::{ShelfRow, ShelfTarget},
    listing::Source,
};
use crate::{
    cover::{self, Slot},
    spotify::{self, Catalog, PlaylistDetails},
};
use std::{rc::Rc, time::Duration};

/// The longest the tracks of a playlist wait for its header, so that both show together.
const HEADER_WAIT: Duration = Duration::from_millis(600);
/// How long the sidebar rests on a playlist before its header loads ahead of Enter.
const PREFETCH_DELAY: Duration = Duration::from_millis(150);

/// A playlist's header, ready to show: its details and its cover's pixels.
#[derive(Clone)]
pub(crate) struct Header {
    details: Rc<PlaylistDetails>,
    pixels: Option<Rc<[u8]>>,
}

impl Jukebox {
    /// Shows the header of the playlist `uri`, holding its tracks back until the
    /// header is ready or [`HEADER_WAIT`] has passed.
    pub(super) fn load_playlist_header(self: &Rc<Self>, uri: &str) {
        let cached = self.headers.borrow().get(uri).cloned();
        if let Some(header) = cached {
            self.header_pending.set(false);
            self.apply_header(&header);
            return;
        }
        self.header_pending.set(true);
        self.fetch_header(uri);
        let (jukebox, uri) = (self.clone(), uri.to_owned());
        self.after(HEADER_WAIT, move || {
            if jukebox.listed_playlist().as_deref() == Some(uri.as_str()) {
                jukebox.header_pending.set(false);
            }
        });
    }

    /// Loads a playlist's header ahead of Enter, once the sidebar rests on it.
    pub(super) fn focus_shelf(self: &Rc<Self>, row: &ShelfRow) {
        self.shelves_focused.set(true);
        let ShelfTarget::Playlist(playlist) = &row.target else {
            return;
        };
        let uri = playlist.uri.clone();
        self.focused_playlist.replace(Some(uri.clone()));
        let jukebox = self.clone();
        self.after(PREFETCH_DELAY, move || {
            if jukebox.focused_playlist.borrow().as_deref() == Some(uri.as_str()) {
                jukebox.fetch_header(&uri);
            }
        });
    }

    /// Loads a header into the cache, unless it is there or on its way.
    fn fetch_header(self: &Rc<Self>, uri: &str) {
        if self.headers.borrow().contains_key(uri)
            || !self.fetching_headers.borrow_mut().insert(uri.to_owned())
        {
            return;
        }
        let Some(catalog) = self.catalog() else {
            return;
        };
        let (jukebox, http, uri) = (self.clone(), self.http.clone(), uri.to_owned());
        let started = self.services.spawn(&self.owner, async move {
            let result = header(catalog, http, uri.clone()).await;
            jukebox.header_arrived(uri, result);
        });
        if let Err(error) = started {
            self.warn(error.to_string());
        }
    }

    /// Keeps a loaded header, and shows it if its playlist is the listed one.
    fn header_arrived(&self, uri: String, result: Result<Header, spotify::Error>) {
        self.fetching_headers.borrow_mut().remove(&uri);
        let listed = self.listed_playlist().as_deref() == Some(uri.as_str());
        match result {
            Ok(header) => {
                self.headers.borrow_mut().insert(uri, header.clone());
                if listed {
                    self.apply_header(&header);
                    self.header_pending.set(false);
                }
            }
            // A header loaded ahead of time fails quietly; the open playlist says why.
            Err(error) if listed => {
                self.header_pending.set(false);
                self.warn(error.to_string());
            }
            Err(_) => {}
        }
    }

    /// Shows `header`, sending its cover to the terminal unless it is already there.
    fn apply_header(&self, header: &Header) {
        self.playlist.set(Some(header.details.clone()));
        let (Some(url), Some(pixels)) = (&header.details.cover, &header.pixels) else {
            return;
        };
        if self
            .playlist_cover
            .with_untracked(|shown| shown.as_ref() == Some(url))
        {
            return;
        }
        match cover::show(Slot::Playlist, pixels) {
            Ok(()) => self.playlist_cover.set(Some(url.clone())),
            Err(error) => self.warn(format!("Cannot read the album cover ({error})")),
        }
    }

    /// The URI of the listed playlist, without tracking it.
    fn listed_playlist(&self) -> Option<String> {
        self.listing
            .with_untracked(|listing| match &listing.source {
                Source::Playlist(playlist) => Some(playlist.uri.clone()),
                _ => None,
            })
    }

    /// Runs `action` after `delay`, unless the app is gone by then.
    fn after(&self, delay: Duration, action: impl FnOnce() + 'static) {
        let Ok(sleep) = self.services.sleep(delay) else {
            return;
        };
        let started = self.services.spawn(&self.owner, async move {
            if sleep.await.is_ok() {
                action();
            }
        });
        if let Err(error) = started {
            self.warn(error.to_string());
        }
    }

    /// Whether the tracks of the listed playlist wait for its header.
    pub(super) fn holding_tracks(&self) -> bool {
        self.header_pending.get()
            && self
                .listing
                .with(|listing| matches!(listing.source, Source::Playlist(_)))
    }

    /// The details of the listed playlist, once they are known.
    fn playlist_header(&self) -> Option<Rc<PlaylistDetails>> {
        let uri = self.listing.with(|listing| match &listing.source {
            Source::Playlist(playlist) => Some(playlist.uri.clone()),
            _ => None,
        })?;
        self.playlist
            .with(|details| details.clone().filter(|details| details.uri == uri))
    }

    pub(super) fn has_playlist_header(&self) -> bool {
        self.playlist_header().is_some()
    }

    pub(super) fn playlist_cover_shown(&self) -> bool {
        cover::supported() && self.has_playlist_header()
    }

    /// One row of the playlist's cover, or blank cells until the terminal has it.
    pub(super) fn playlist_cover_row(&self, row: usize) -> String {
        let wanted = self
            .playlist_header()
            .and_then(|details| details.cover.clone());
        if wanted.is_some() && self.playlist_cover.with(|shown| *shown == wanted) {
            cover::row(Slot::Playlist, row)
        } else {
            " ".repeat(Slot::Playlist.columns())
        }
    }

    pub(super) fn playlist_name(&self) -> String {
        self.playlist_header()
            .map(|details| details.name.clone())
            .unwrap_or_default()
    }

    pub(super) fn playlist_description(&self) -> String {
        self.playlist_header()
            .map(|details| details.description.clone())
            .unwrap_or_default()
    }

    /// Who made the playlist and how many tracks it holds.
    pub(super) fn playlist_byline(&self) -> String {
        let Some(details) = self.playlist_header() else {
            return String::new();
        };
        let summary = self.listing_summary();
        [details.owner.clone(), summary]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// A playlist's details and cover; a cover that fails to load leaves the header without it.
async fn header(
    catalog: Catalog,
    http: reqwest::Client,
    uri: String,
) -> Result<Header, spotify::Error> {
    let details = catalog.playlist_details(uri).await?;
    let pixels = match details.cover.clone().filter(|_| cover::supported()) {
        Some(url) => cover::fetch(http, url, Slot::Playlist)
            .await
            .ok()
            .map(Rc::from),
        None => None,
    };
    Ok(Header {
        details: Rc::new(details),
        pixels,
    })
}
