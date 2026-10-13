use super::{Jukebox, listing::Source};
use crate::{
    cover::{self, Slot},
    spotify::{Error as SpotifyError, PlaylistDetails},
};
use std::rc::Rc;

impl Jukebox {
    /// Loads the header of the playlist `uri`, unless it is already there.
    pub(super) fn load_playlist_header(self: &Rc<Self>, uri: &str) {
        let known = self
            .playlist
            .with_untracked(|details| details.as_ref().is_some_and(|details| details.uri == uri));
        let Some(catalog) = self.catalog() else {
            return;
        };
        if known {
            return;
        }
        let (http, playlist, shown, uri) = (
            self.http.clone(),
            self.playlist.clone(),
            self.playlist_cover.clone(),
            uri.to_owned(),
        );
        let started = self.spawn(async move {
            let details = Rc::new(catalog.playlist_details(uri).await?);
            playlist.set(Some(details.clone()));
            let Some(url) = details.cover.clone().filter(|_| cover::supported()) else {
                return Ok(());
            };
            if shown.with_untracked(|shown| shown.as_ref() == Some(&url)) {
                return Ok(());
            }
            let pixels = cover::fetch(http, url.clone(), Slot::Playlist).await?;
            // Another playlist may have opened during the download.
            let current = playlist
                .with_untracked(|current| current.as_ref().map(|current| current.uri.clone()));
            if current.as_deref() == Some(details.uri.as_str()) {
                cover::show(Slot::Playlist, &pixels)
                    .map_err(|error| SpotifyError::Cover(error.to_string()))?;
                shown.set(Some(url));
            }
            Ok(())
        });
        if let Err(error) = started {
            self.warn(error.to_string());
        }
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
