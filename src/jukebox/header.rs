//! The header above the tracks of a playlist or an artist: picture, name and details.
use super::{
    Jukebox,
    artist::{ArtistCache, artist_page},
    library::{ShelfRow, ShelfTarget},
    listing::Source,
};
use crate::{
    cover::{self, Slot},
    spotify::{self, ArtistPage, Catalog, PlaylistDetails},
};
use std::{rc::Rc, time::Duration};

/// The longest the tracks of a page wait for its header, so that both show together.
const HEADER_WAIT: Duration = Duration::from_millis(600);
/// How long the sidebar rests on a playlist before its header loads ahead of Enter.
const PREFETCH_DELAY: Duration = Duration::from_millis(150);
const ARTIST_PREFIX: &str = "spotify:artist:";

/// What the header shows: the picture, a kind, the name and two lines of details.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeaderInfo {
    pub(crate) uri: String,
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) detail: String,
    pub(crate) byline: String,
    /// Whether the byline ends with the number of tracks, as a playlist's does.
    pub(crate) counts: bool,
    pub(crate) cover: Option<String>,
}

/// A header ready to show: its text and its picture's pixels.
#[derive(Clone)]
pub(crate) struct Header {
    info: Rc<HeaderInfo>,
    pixels: Option<Rc<[u8]>>,
}

impl HeaderInfo {
    fn playlist(details: PlaylistDetails) -> Self {
        Self {
            uri: details.uri,
            kind: "PLAYLIST".into(),
            name: details.name,
            detail: details.description,
            byline: details.owner,
            counts: true,
            cover: details.cover,
        }
    }

    fn artist(page: &ArtistPage) -> Self {
        let stats = [
            page.monthly_listeners
                .map(|count| format!("{} monthly listeners", grouped(count))),
            page.followers
                .map(|count| format!("{} followers", grouped(count))),
            page.world_rank.map(|rank| format!("#{rank} in the world")),
        ];
        Self {
            uri: page.uri.clone(),
            kind: if page.verified {
                "ARTIST · ✓ Verified".into()
            } else {
                "ARTIST".into()
            },
            name: page.name.clone(),
            detail: stats.into_iter().flatten().collect::<Vec<_>>().join(" · "),
            byline: page.biography.clone(),
            counts: false,
            cover: page.avatar.clone(),
        }
    }
}

/// A count with a comma between thousands: 28,201,141.
pub(super) fn grouped(count: u64) -> String {
    let digits = count.to_string();
    let mut text = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            text.push(',');
        }
        text.push(digit);
    }
    text
}

impl Jukebox {
    /// Shows the header of the page `uri`, holding its tracks back until the
    /// header is ready or [`HEADER_WAIT`] has passed.
    pub(super) fn load_header(self: &Rc<Self>, uri: &str) {
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
            if jukebox.listed_page().as_deref() == Some(uri.as_str()) {
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
        let (jukebox, http, artists, uri) = (
            self.clone(),
            self.http.clone(),
            self.artists.clone(),
            uri.to_owned(),
        );
        let started = self.services.spawn(&self.owner, async move {
            let result = header(catalog, http, artists, uri.clone()).await;
            jukebox.header_arrived(uri, result);
        });
        if let Err(error) = started {
            self.warn(error.to_string());
        }
    }

    /// Keeps a loaded header, and shows it if its page is the listed one.
    fn header_arrived(&self, uri: String, result: Result<Header, spotify::Error>) {
        self.fetching_headers.borrow_mut().remove(&uri);
        let listed = self.listed_page().as_deref() == Some(uri.as_str());
        match result {
            Ok(header) => {
                self.headers.borrow_mut().insert(uri, header.clone());
                if listed {
                    self.apply_header(&header);
                    self.header_pending.set(false);
                }
            }
            // A header loaded ahead of time fails quietly; the open page says why.
            Err(error) if listed => {
                self.header_pending.set(false);
                self.warn(error.to_string());
            }
            Err(_) => {}
        }
    }

    /// Shows `header`, sending its picture to the terminal unless it is already there.
    fn apply_header(&self, header: &Header) {
        self.header.set(Some(header.info.clone()));
        let (Some(url), Some(pixels)) = (&header.info.cover, &header.pixels) else {
            return;
        };
        if self
            .header_cover
            .with_untracked(|shown| shown.as_ref() == Some(url))
        {
            return;
        }
        match cover::show(Slot::Header, pixels) {
            Ok(()) => self.header_cover.set(Some(url.clone())),
            Err(error) => self.warn(format!("Cannot read the album cover ({error})")),
        }
    }

    /// The URI of the listed playlist or artist, without tracking it.
    fn listed_page(&self) -> Option<String> {
        self.listing
            .with_untracked(|listing| page_uri(&listing.source))
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

    /// Whether the tracks of the listed page wait for its header.
    pub(super) fn holding_tracks(&self) -> bool {
        self.header_pending.get()
            && self
                .listing
                .with(|listing| page_uri(&listing.source).is_some())
    }

    /// The header of the listed page, once it is known.
    fn listed_header(&self) -> Option<Rc<HeaderInfo>> {
        let uri = self.listing.with(|listing| page_uri(&listing.source))?;
        self.header
            .with(|header| header.clone().filter(|header| header.uri == uri))
    }

    pub(super) fn header_shown(&self) -> bool {
        self.listed_header().is_some()
    }

    pub(super) fn header_cover_shown(&self) -> bool {
        cover::supported() && self.header_shown()
    }

    /// One row of the header's picture, or blank cells until the terminal has it.
    pub(super) fn header_cover_row(&self, row: usize) -> String {
        let wanted = self.listed_header().and_then(|header| header.cover.clone());
        if wanted.is_some() && self.header_cover.with(|shown| *shown == wanted) {
            cover::row(Slot::Header, row)
        } else {
            " ".repeat(Slot::Header.columns())
        }
    }

    pub(super) fn header_kind(&self) -> String {
        self.header_text(|header| header.kind.clone())
    }

    pub(super) fn header_name(&self) -> String {
        self.header_text(|header| header.name.clone())
    }

    pub(super) fn header_detail(&self) -> String {
        self.header_text(|header| header.detail.clone())
    }

    /// Who made the playlist and how many tracks it holds, or the artist's biography.
    pub(super) fn header_byline(&self) -> String {
        let Some(header) = self.listed_header() else {
            return String::new();
        };
        let summary = if header.counts {
            self.listing_summary()
        } else {
            String::new()
        };
        [header.byline.clone(), summary]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }

    fn header_text(&self, text: impl FnOnce(&HeaderInfo) -> String) -> String {
        self.listed_header()
            .map(|header| text(&header))
            .unwrap_or_default()
    }
}

/// The URI of the page a source has a header for.
fn page_uri(source: &Source) -> Option<String> {
    match source {
        Source::Playlist(playlist) => Some(playlist.uri.clone()),
        Source::Artist(artist) => Some(artist.uri.clone()),
        _ => None,
    }
}

/// A page's header and picture; a picture that fails to load leaves the header without it.
async fn header(
    catalog: Catalog,
    http: reqwest::Client,
    artists: ArtistCache,
    uri: String,
) -> Result<Header, spotify::Error> {
    let info = if uri.starts_with(ARTIST_PREFIX) {
        let page = artist_page(&catalog, &artists, uri).await?;
        HeaderInfo::artist(&page)
    } else {
        HeaderInfo::playlist(catalog.playlist_details(uri).await?)
    };
    let pixels = match info.cover.clone().filter(|_| cover::supported()) {
        Some(url) => cover::fetch(http, url, Slot::Header)
            .await
            .ok()
            .map(Rc::from),
        None => None,
    };
    Ok(Header {
        info: Rc::new(info),
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::grouped;

    #[test]
    fn counts_group_their_thousands() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000), "1,000");
        assert_eq!(grouped(28_201_141), "28,201,141");
    }
}
