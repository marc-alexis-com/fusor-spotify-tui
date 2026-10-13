use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Artist {
    pub(crate) uri: String,
    pub(crate) name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Album {
    pub(crate) uri: String,
    pub(crate) name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Track {
    pub(crate) uri: String,
    pub(crate) name: String,
    pub(crate) artists: Vec<Artist>,
    pub(crate) album: Album,
    pub(crate) duration_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Playlist {
    pub(crate) uri: String,
    pub(crate) name: String,
}

/// What the header of a playlist shows, besides its number of tracks.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlaylistDetails {
    pub(crate) uri: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) owner: String,
    pub(crate) cover: Option<String>,
}

/// A playlist or a folder of the user's library.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LibraryItem {
    Playlist(Rc<Playlist>),
    Folder { id: String, name: String },
}

/// One entry of the library, in the order Spotify shows it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LibraryEntry {
    pub(crate) item: LibraryItem,
    /// The IDs of the folders it sits in, outermost first.
    pub(crate) folders: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Repeat {
    #[default]
    Off,
    Context,
    Track,
}

impl Track {
    pub(crate) fn artist_names(&self) -> String {
        self.artists
            .iter()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl Repeat {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Off => Self::Context,
            Self::Context => Self::Track,
            Self::Track => Self::Off,
        }
    }
}
