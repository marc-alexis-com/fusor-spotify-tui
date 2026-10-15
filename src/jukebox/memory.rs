//! The view the player showed last, which it opens again at the next start. It
//! lives in `$XDG_STATE_HOME/spt/view.json`: losing it only means opening Liked Songs.
use super::listing::Source;
use crate::spotify::{Album, Artist, Playlist};
use serde::{Deserialize, Serialize};
use std::{io, path::PathBuf, rc::Rc};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum View {
    Liked,
    Recent,
    Search { text: String },
    Library { text: String },
    Playlist { uri: String, name: String },
    Album { uri: String, name: String },
    Artist { uri: String, name: String },
    Show { uri: String, name: String },
}

impl From<&Source> for View {
    fn from(source: &Source) -> Self {
        match source {
            Source::Liked => Self::Liked,
            Source::Recent => Self::Recent,
            Source::Search(text) => Self::Search { text: text.clone() },
            Source::Library(text) => Self::Library { text: text.clone() },
            Source::Playlist(playlist) => Self::Playlist {
                uri: playlist.uri.clone(),
                name: playlist.name.clone(),
            },
            Source::Album(album) => Self::Album {
                uri: album.uri.clone(),
                name: album.name.clone(),
            },
            Source::Artist(artist) => Self::Artist {
                uri: artist.uri.clone(),
                name: artist.name.clone(),
            },
            Source::Show(show) => Self::Show {
                uri: show.uri.clone(),
                name: show.name.clone(),
            },
        }
    }
}

impl From<View> for Source {
    fn from(view: View) -> Self {
        match view {
            View::Liked => Self::Liked,
            View::Recent => Self::Recent,
            View::Search { text } => Self::Search(text),
            View::Library { text } => Self::Library(text),
            View::Playlist { uri, name } => Self::Playlist(Rc::new(Playlist { uri, name })),
            View::Album { uri, name } => Self::Album(Rc::new(Album { uri, name })),
            View::Artist { uri, name } => Self::Artist(Rc::new(Artist { uri, name })),
            View::Show { uri, name } => Self::Show(Rc::new(Album { uri, name })),
        }
    }
}

fn path() -> Option<PathBuf> {
    crate::config::xdg_dir("XDG_STATE_HOME", ".local/state").map(|dir| dir.join("view.json"))
}

/// The view saved last, if any could be read.
pub(super) fn recall() -> Option<Source> {
    let text = std::fs::read_to_string(path()?).ok()?;
    serde_json::from_str::<View>(&text).ok().map(Source::from)
}

/// Saves the view, whole or not at all: it goes to a file beside, then takes its name.
pub(super) fn remember(source: &Source) -> io::Result<()> {
    let path = path().ok_or_else(|| io::Error::other("no home folder to save the view in"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let partial = path.with_extension("json.partial");
    std::fs::write(&partial, serde_json::to_vec(&View::from(source))?)?;
    std::fs::rename(partial, path)
}

#[cfg(test)]
mod tests {
    use super::{Source, View};
    use crate::spotify::Playlist;
    use std::rc::Rc;

    #[test]
    fn a_view_comes_back_as_it_left() {
        let sources = [
            Source::Liked,
            Source::Search("daft punk".into()),
            Source::Playlist(Rc::new(Playlist {
                uri: "spotify:playlist:37i9dQZF1DXcBWIGoYBM5M".into(),
                name: "Today's Top Hits".into(),
            })),
        ];
        for source in sources {
            let text = serde_json::to_string(&View::from(&source)).unwrap();
            let view: View = serde_json::from_str(&text).unwrap();
            assert_eq!(Source::from(view), source);
        }
    }
}
