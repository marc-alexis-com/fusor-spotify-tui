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
    /// When an episode came out, as `YYYY-MM-DD`; tracks have none.
    pub(crate) released: Option<String>,
    /// Whether Spotify can play it for this account, here and now.
    pub(crate) playable: bool,
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

/// An artist's page: who they are, how many listen, and what they released.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ArtistPage {
    pub(crate) uri: String,
    pub(crate) name: String,
    pub(crate) verified: bool,
    pub(crate) monthly_listeners: Option<u64>,
    pub(crate) followers: Option<u64>,
    pub(crate) world_rank: Option<u64>,
    pub(crate) biography: String,
    pub(crate) avatar: Option<String>,
    /// The popular tracks and how often they were played, when Spotify says.
    pub(crate) top_tracks: Vec<(String, Option<u64>)>,
    pub(crate) albums: Vec<Rc<SearchItem>>,
    pub(crate) singles: Vec<Rc<SearchItem>>,
    pub(crate) appears_on: Vec<Rc<SearchItem>>,
    pub(crate) related: Vec<Rc<SearchItem>>,
}

/// What a search result opens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SearchKind {
    Artist,
    Album,
    Playlist,
    Podcast,
}

/// An artist, album, playlist or podcast found by a search.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SearchItem {
    pub(crate) kind: SearchKind,
    pub(crate) uri: String,
    pub(crate) name: String,
    /// Who made it: the album's artists, the playlist's owner, the podcast's publisher.
    pub(crate) detail: String,
}

/// The sections of a search besides its songs.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SearchResults {
    pub(crate) artists: Vec<Rc<SearchItem>>,
    pub(crate) albums: Vec<Rc<SearchItem>>,
    pub(crate) playlists: Vec<Rc<SearchItem>>,
    pub(crate) podcasts: Vec<Rc<SearchItem>>,
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
