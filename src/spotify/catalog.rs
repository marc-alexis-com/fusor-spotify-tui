use super::{Album, Artist, Error, LibraryEntry, LibraryItem, Playlist, PlaylistDetails, Track};
use http::Method;
use librespot_core::{Session, SpotifyId, SpotifyUri};
use librespot_protocol::{
    extended_metadata::{BatchedEntityRequest, EntityRequest, ExtensionQuery},
    extension_kind::ExtensionKind,
    metadata,
    playlist4_external::SelectedListContent,
};
use protobuf::{EnumOrUnknown, Message};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
    rc::Rc,
};

const ROOTLIST_LENGTH: usize = 1000;
const TRACK_PREFIX: &str = "spotify:track:";
const EPISODE_PREFIX: &str = "spotify:episode:";
const PLAYLIST_PREFIX: &str = "spotify:playlist:";
/// Folders appear in the rootlist as a start marker, their contents, then an end marker.
const FOLDER_START: &str = "spotify:start-group:";
const FOLDER_END: &str = "spotify:end-group:";
const UNNAMED_FOLDER: &str = "Untitled folder";
const SEARCH_PREFIX: &str = "spotify:search:";
const STATUS_OK: i32 = 200;
const IMAGE_URL: &str = "https://i.scdn.co/image/";
/// Spotify's own mosaic of four covers, for playlists without a picture.
const MOSAIC_URL: &str = "https://mosaic.scdn.co/300/";
const MOSAIC_TILES: usize = 4;
/// Tracks whose covers may make up a mosaic.
const MOSAIC_CANDIDATES: usize = 30;
/// Editorial playlists name their picture among their format attributes.
const IMAGE_ATTRIBUTE: &str = "image_url";

#[derive(Deserialize)]
struct Profile {
    name: Option<String>,
}

/// Reads the library and catalog through the same session that plays audio.
#[derive(Clone)]
pub(crate) struct Catalog {
    session: Session,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecentlyPlayed {
    play_contexts: Vec<PlayContext>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayContext {
    last_played_track_uri: Option<String>,
}

impl Catalog {
    pub(crate) fn new(session: Session) -> Self {
        Self { session }
    }

    pub(crate) fn liked_uri(&self) -> String {
        format!("spotify:user:{}:collection", self.session.username())
    }

    /// The playlists and folders of the library, folders included in place.
    pub(crate) async fn library(&self) -> Result<Vec<LibraryEntry>, Error> {
        let session = self.session.clone();
        let entries = run(async move {
            let bytes = session
                .spclient()
                .get_rootlist(0, Some(ROOTLIST_LENGTH))
                .await?;
            let rootlist = SelectedListContent::parse_from_bytes(&bytes)?;
            let contents = &rootlist.contents;
            Ok(contents
                .items
                .iter()
                .zip(&contents.meta_items)
                .map(|(item, meta)| (item.uri().to_owned(), meta.attributes.name.clone()))
                .collect::<Vec<_>>())
        })
        .await?;
        Ok(library_entries(entries))
    }

    /// The tracks and episodes of a playlist. Spotify's context of a playlist leaves its
    /// episodes out, and is empty for a playlist of podcasts, so the playlist itself is read.
    pub(crate) async fn playlist_tracks(&self, uri: String) -> Result<Vec<String>, Error> {
        let session = self.session.clone();
        run(async move {
            let id = SpotifyId::from_base62(uri.strip_prefix(PLAYLIST_PREFIX).unwrap_or(&uri))?;
            let bytes = session.spclient().get_playlist(&id).await?;
            let playlist = SelectedListContent::parse_from_bytes(&bytes)?;
            let uris = playlist
                .contents
                .items
                .iter()
                .map(|item| item.uri().to_owned());
            Ok(unique_tracks(uris))
        })
        .await
    }

    /// The name, description, owner and cover of a playlist.
    pub(crate) async fn playlist_details(&self, uri: String) -> Result<PlaylistDetails, Error> {
        let session = self.session.clone();
        run(async move {
            let id = SpotifyId::from_base62(uri.strip_prefix(PLAYLIST_PREFIX).unwrap_or(&uri))?;
            let bytes = session.spclient().get_playlist(&id).await?;
            let playlist = SelectedListContent::parse_from_bytes(&bytes)?;
            let attributes = &playlist.attributes;
            let picture = attributes
                .picture
                .as_deref()
                .map(|picture| format!("{IMAGE_URL}{}", hex(picture)))
                .or_else(|| {
                    attributes
                        .format_attributes
                        .iter()
                        .find(|attribute| attribute.key() == IMAGE_ATTRIBUTE)
                        .map(|attribute| attribute.value().to_owned())
                });
            let cover = match picture {
                Some(picture) => Some(picture),
                None => {
                    let uris = playlist
                        .contents
                        .items
                        .iter()
                        .map(|item| item.uri().to_owned());
                    let candidates = unique_tracks(uris).into_iter().take(MOSAIC_CANDIDATES);
                    mosaic(&session, candidates.collect()).await?
                }
            };
            let username = playlist.owner_username().to_owned();
            let owner = owner_name(&session, &username).await.unwrap_or(username);
            Ok(PlaylistDetails {
                uri,
                name: attributes.name().to_owned(),
                description: plain_text(attributes.description()),
                owner,
                cover,
            })
        })
        .await
    }

    /// The tracks of a playlist, album, artist, show, the liked songs or a search.
    pub(crate) async fn context_tracks(&self, uri: String) -> Result<Vec<String>, Error> {
        let session = self.session.clone();
        run(async move {
            let context = session.spclient().get_context(&uri).await?;
            let uris = context
                .pages
                .iter()
                .flat_map(|page| &page.tracks)
                .map(|track| track.uri().to_owned());
            Ok(unique_tracks(uris))
        })
        .await
    }

    /// The last track played in each recently played album, playlist or artist.
    pub(crate) async fn recent_tracks(&self) -> Result<Vec<String>, Error> {
        let session = self.session.clone();
        run(async move {
            let path = format!(
                "/recently-played/v3/user/{}/recently-played?format=json&offset=0&limit=50&filter=default",
                session.username()
            );
            let body = session
                .spclient()
                .request_as_json(&Method::GET, &path, None, None)
                .await?;
            let recent: RecentlyPlayed = serde_json::from_slice(&body)?;
            let uris = recent
                .play_contexts
                .into_iter()
                .filter_map(|context| context.last_played_track_uri);
            Ok(unique_tracks(uris))
        })
        .await
    }

    /// Looks up many tracks and episodes in one request, in the order of `uris`;
    /// unavailable ones are left out.
    pub(crate) async fn tracks(&self, uris: Vec<String>) -> Result<Vec<Track>, Error> {
        let session = self.session.clone();
        run(async move {
            let request = BatchedEntityRequest {
                entity_request: uris.iter().map(|uri| track_request(uri)).collect(),
                ..BatchedEntityRequest::default()
            };
            let response = session.spclient().get_extended_metadata(request).await?;
            // A track Spotify describes incompletely is left out like an unavailable one,
            // instead of failing the whole batch and stopping the listing there.
            let mut found: HashMap<_, _> = response
                .extended_metadata
                .iter()
                .flat_map(|array| &array.extension_data)
                .filter(|data| data.header.status_code == STATUS_OK)
                .filter_map(|data| {
                    let uri = &data.entity_uri;
                    Some((uri.clone(), describe(uri, &data.extension_data.value)?))
                })
                .collect();
            Ok(uris.iter().filter_map(|uri| found.remove(uri)).collect())
        })
        .await
    }
}

/// The context URI that searches the catalog for `text`.
pub(crate) fn search_uri(text: &str) -> String {
    let mut uri = String::from(SEARCH_PREFIX);
    for byte in text.trim().bytes() {
        match byte {
            b' ' => uri.push('+'),
            _ if byte.is_ascii_alphanumeric() => uri.push(char::from(byte)),
            _ => {
                write!(uri, "%{byte:02X}").expect("writing to a String cannot fail");
            }
        }
    }
    uri
}

/// librespot's requests are `Send`; they run on the Tokio runtime, not the UI thread.
async fn run<T: Send + 'static>(
    task: impl Future<Output = Result<T, Error>> + Send + 'static,
) -> Result<T, Error> {
    tokio::spawn(task).await?
}

/// Nests the rootlist's playlists in the folders its start and end markers delimit.
fn library_entries(items: Vec<(String, Option<String>)>) -> Vec<LibraryEntry> {
    let mut folders: Vec<String> = Vec::new();
    let mut entries = Vec::new();
    for (uri, name) in items {
        if let Some(folder) = uri.strip_prefix(FOLDER_START) {
            let (id, encoded) = folder.split_once(':').unwrap_or((folder, ""));
            let name = decode_name(encoded);
            entries.push(LibraryEntry {
                item: LibraryItem::Folder {
                    id: id.to_owned(),
                    name: if name.is_empty() {
                        UNNAMED_FOLDER.into()
                    } else {
                        name
                    },
                },
                folders: folders.clone(),
            });
            folders.push(id.to_owned());
        } else if uri.starts_with(FOLDER_END) {
            folders.pop();
        } else if uri.starts_with(PLAYLIST_PREFIX)
            && let Some(name) = name
        {
            entries.push(LibraryEntry {
                item: LibraryItem::Playlist(Rc::new(Playlist { uri, name })),
                folders: folders.clone(),
            });
        }
    }
    entries
}

/// Folder names are URL-encoded in their marker, with `+` for spaces.
fn decode_name(encoded: &str) -> String {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        let escaped = encoded
            .get(index + 1..index + 3)
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (byte, escaped) {
            (b'+', _) => decoded.push(b' '),
            (b'%', Some(value)) => {
                decoded.push(value);
                index += 2;
            }
            _ => decoded.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// The tracks and episodes among `uris`, each once, in order.
fn unique_tracks(uris: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    uris.filter(|uri| {
        (uri.starts_with(TRACK_PREFIX) || uri.starts_with(EPISODE_PREFIX))
            && seen.insert(uri.clone())
    })
    .collect()
}

fn track_request(uri: &str) -> EntityRequest {
    let kind = if uri.starts_with(EPISODE_PREFIX) {
        ExtensionKind::EPISODE_V4
    } else {
        ExtensionKind::TRACK_V4
    };
    EntityRequest {
        entity_uri: uri.to_owned(),
        query: vec![ExtensionQuery {
            extension_kind: EnumOrUnknown::new(kind),
            ..ExtensionQuery::default()
        }],
        ..EntityRequest::default()
    }
}

fn track(uri: &str, message: &metadata::Track) -> Result<Track, Error> {
    let artists = message
        .artist
        .iter()
        .map(|artist| {
            Ok(Artist {
                uri: SpotifyUri::try_from(artist)?.to_uri()?,
                name: artist.name().to_owned(),
            })
        })
        .collect::<Result<_, Error>>()?;
    Ok(Track {
        uri: uri.to_owned(),
        name: message.name().to_owned(),
        artists,
        album: Album {
            uri: SpotifyUri::try_from(&*message.album)?.to_uri()?,
            name: message.album.name().to_owned(),
        },
        duration_ms: u64::from(message.duration().unsigned_abs()),
    })
}

/// The display name of a user, which their username is not always meant to be read as.
async fn owner_name(session: &Session, username: &str) -> Option<String> {
    let profile = session
        .spclient()
        .get_user_profile(username, Some(0), Some(0))
        .await
        .ok()?;
    let profile: Profile = serde_json::from_slice(&profile).ok()?;
    profile.name.filter(|name| !name.is_empty())
}

/// The cover Spotify shows for a playlist without a picture: a mosaic of the first
/// four different covers of its tracks, or the only one there is.
async fn mosaic(session: &Session, uris: Vec<String>) -> Result<Option<String>, Error> {
    if uris.is_empty() {
        return Ok(None);
    }
    let request = BatchedEntityRequest {
        entity_request: uris.iter().map(|uri| track_request(uri)).collect(),
        ..BatchedEntityRequest::default()
    };
    let response = session.spclient().get_extended_metadata(request).await?;
    let mut found: HashMap<_, _> = response
        .extended_metadata
        .iter()
        .flat_map(|array| &array.extension_data)
        .filter(|data| data.header.status_code == STATUS_OK)
        .filter_map(|data| {
            let cover = cover_id(&data.entity_uri, &data.extension_data.value)?;
            Some((data.entity_uri.clone(), cover))
        })
        .collect();
    let mut covers: Vec<String> = Vec::new();
    for cover in uris.iter().filter_map(|uri| found.remove(uri)) {
        if !covers.contains(&cover) {
            covers.push(cover);
        }
    }
    Ok(match covers.len() {
        0 => None,
        count if count < MOSAIC_TILES => covers.first().map(|cover| format!("{IMAGE_URL}{cover}")),
        _ => Some(format!("{MOSAIC_URL}{}", covers[..MOSAIC_TILES].concat())),
    })
}

/// The hexadecimal ID of the default-size cover of a track's album or of an episode.
fn cover_id(uri: &str, bytes: &[u8]) -> Option<String> {
    let images = if uri.starts_with(EPISODE_PREFIX) {
        metadata::Episode::parse_from_bytes(bytes)
            .ok()?
            .cover_image
            .image
            .clone()
    } else {
        metadata::Track::parse_from_bytes(bytes)
            .ok()?
            .album
            .cover_group
            .image
            .clone()
    };
    let image = images
        .iter()
        .find(|image| image.size() == metadata::image::Size::DEFAULT)
        .or(images.first())?;
    Some(hex(image.file_id()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        write!(text, "{byte:02x}").expect("writing to a String cannot fail");
        text
    })
}

/// Playlist descriptions are HTML: links are kept as their text, entities decoded.
fn plain_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut characters = html.chars().peekable();
    while let Some(character) = characters.next() {
        // A tag starts with a letter or a slash; a lone `<` is text.
        let opens_tag = character == '<'
            && characters
                .peek()
                .is_some_and(|next| next.is_ascii_alphabetic() || *next == '/');
        match character {
            _ if opens_tag => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    [
        ("&quot;", "\""),
        ("&#x27;", "'"),
        ("&#39;", "'"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&amp;", "&"),
    ]
    .iter()
    .fold(text, |text, (entity, character)| {
        text.replace(entity, character)
    })
}

/// The track or episode that `bytes` describe, unless Spotify described it incompletely.
fn describe(uri: &str, bytes: &[u8]) -> Option<Track> {
    let found = if uri.starts_with(EPISODE_PREFIX) {
        episode(uri, &metadata::Episode::parse_from_bytes(bytes).ok()?)
    } else {
        track(uri, &metadata::Track::parse_from_bytes(bytes).ok()?)
    };
    found.ok()
}

/// An episode lists like a track whose album is its show, and has no artists.
fn episode(uri: &str, message: &metadata::Episode) -> Result<Track, Error> {
    Ok(Track {
        uri: uri.to_owned(),
        name: message.name().to_owned(),
        artists: Vec::new(),
        album: Album {
            uri: SpotifyUri::try_from(&*message.show)?.to_uri()?,
            name: message.show.name().to_owned(),
        },
        duration_ms: u64::from(message.duration().unsigned_abs()),
    })
}

#[cfg(test)]
mod tests {
    use super::{search_uri, unique_tracks};
    use crate::spotify::{LibraryEntry, LibraryItem};

    fn entry(uri: &str, name: Option<&str>) -> (String, Option<String>) {
        (uri.to_owned(), name.map(str::to_owned))
    }

    #[test]
    fn descriptions_lose_their_html() {
        assert_eq!(
            super::plain_text(
                "Chill beats with <a href=\"spotify:playlist:x\">Lofi Girl</a> &amp; friends. It&#x27;s &quot;on&quot;"
            ),
            "Chill beats with Lofi Girl & friends. It's \"on\""
        );
        assert_eq!(super::plain_text("1 < 2"), "1 < 2");
    }

    #[test]
    fn ids_print_as_hexadecimal() {
        assert_eq!(super::hex(&[0xab, 0x67, 0x00, 0x0f]), "ab67000f");
    }

    #[test]
    fn folder_names_are_decoded() {
        assert_eq!(
            super::decode_name(
                "%CF%88%CF%85%CF%87%CE%B5%CE%B4%CE%B5%CE%BB%CE%B9%CE%BA%CF%8C%CF%82"
            ),
            "ψυχεδελικός"
        );
        assert_eq!(super::decode_name("Road+trip%21"), "Road trip!");
        assert_eq!(super::decode_name("100%"), "100%");
    }

    #[test]
    fn playlists_nest_in_their_folders() {
        let entries = super::library_entries(vec![
            entry("spotify:playlist:a", Some("A")),
            entry("spotify:start-group:f1:Outer", None),
            entry("spotify:playlist:b", Some("B")),
            entry("spotify:start-group:f2:", None),
            entry("spotify:playlist:c", Some("C")),
            entry("spotify:end-group:f2", None),
            entry("spotify:end-group:f1", None),
            entry("spotify:playlist:d", Some("D")),
            entry("spotify:playlist:unnamed", None),
        ]);
        let summary: Vec<(String, Vec<String>)> = entries
            .iter()
            .map(|LibraryEntry { item, folders }| {
                let name = match item {
                    LibraryItem::Playlist(playlist) => playlist.name.clone(),
                    LibraryItem::Folder { name, .. } => format!("[{name}]"),
                };
                (name, folders.clone())
            })
            .collect();
        let path = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            summary,
            vec![
                ("A".into(), path(&[])),
                ("[Outer]".into(), path(&[])),
                ("B".into(), path(&["f1"])),
                ("[Untitled folder]".into(), path(&["f1"])),
                ("C".into(), path(&["f1", "f2"])),
                ("D".into(), path(&[])),
            ]
        );
    }

    #[test]
    fn search_uris_encode_the_query() {
        assert_eq!(search_uri(" daft punk "), "spotify:search:daft+punk");
        assert_eq!(search_uri("AC/DC & co"), "spotify:search:AC%2FDC+%26+co");
        assert_eq!(search_uri("beyoncé"), "spotify:search:beyonc%C3%A9");
    }

    #[test]
    fn contexts_keep_each_track_once_in_order() {
        let uris = [
            "spotify:track:b",
            "spotify:episode:x",
            "spotify:local:artist:album:name:60",
            "spotify:track:a",
            "spotify:track:b",
            "spotify:episode:x",
        ]
        .map(String::from);
        assert_eq!(
            unique_tracks(uris.into_iter()),
            ["spotify:track:b", "spotify:episode:x", "spotify:track:a"]
        );
    }
}
