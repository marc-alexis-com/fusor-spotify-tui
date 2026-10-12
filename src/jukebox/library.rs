use super::{Jukebox, listing::Source};
use crate::{
    icons::{self, Icons},
    spotify::{LibraryEntry, LibraryItem, Playlist},
};
use std::{collections::HashSet, rc::Rc};

/// Shelves above the playlists in `ui/shelves.html`: Search, Liked Songs, Recently played.
pub(super) const FIXED_SHELVES: usize = 3;
const INDENT: &str = "  ";

/// One line of the sidebar's playlist section.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShelfRow {
    pub(crate) key: String,
    pub(crate) label: String,
    /// The playing list, or a closed folder that holds it.
    pub(crate) current: bool,
    /// The folder this row sits in.
    pub(crate) parent: Option<String>,
    pub(crate) target: ShelfTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ShelfTarget {
    Folder { id: String, open: bool },
    Playlist(Rc<Playlist>),
}

impl ShelfRow {
    pub(crate) fn is_folder(&self) -> bool {
        matches!(self.target, ShelfTarget::Folder { .. })
    }
}

/// The rows of the entries whose folders are all open; folders start closed.
pub(crate) fn shelf_rows(
    entries: &[LibraryEntry],
    open: &HashSet<String>,
    current: Option<&str>,
    icons: &Icons,
) -> Vec<ShelfRow> {
    let holds_current = |folder: &str| {
        entries.iter().any(|entry| {
            matches!(&entry.item, LibraryItem::Playlist(playlist) if Some(playlist.uri.as_str()) == current)
                && entry.folders.iter().any(|id| id == folder)
        })
    };
    entries
        .iter()
        .filter(|entry| entry.folders.iter().all(|id| open.contains(id)))
        .map(|entry| {
            let indent = INDENT.repeat(entry.folders.len());
            let parent = entry.folders.last().cloned();
            match &entry.item {
                LibraryItem::Folder { id, name } => {
                    let is_open = open.contains(id);
                    let marker = if is_open {
                        icons.folder_open
                    } else {
                        icons.folder_closed
                    };
                    ShelfRow {
                        key: format!("folder:{id}"),
                        label: format!("{indent}{marker}{name}"),
                        current: !is_open && holds_current(id),
                        parent,
                        target: ShelfTarget::Folder {
                            id: id.clone(),
                            open: is_open,
                        },
                    }
                }
                LibraryItem::Playlist(playlist) => ShelfRow {
                    key: playlist.uri.clone(),
                    label: format!("{indent}{}{}", icons.playlist, playlist.name),
                    current: Some(playlist.uri.as_str()) == current,
                    parent,
                    target: ShelfTarget::Playlist(playlist.clone()),
                },
            }
        })
        .collect()
}

impl Jukebox {
    pub(super) fn shelf_rows(&self) -> Vec<Rc<ShelfRow>> {
        let current = self.listing.with(|listing| match &listing.source {
            Source::Playlist(playlist) => Some(playlist.uri.clone()),
            _ => None,
        });
        self.open_folders.with(|open| {
            self.library.with(|entries| {
                shelf_rows(entries, open, current.as_deref(), icons::get())
                    .into_iter()
                    .map(Rc::new)
                    .collect()
            })
        })
    }

    pub(super) fn choose_shelf(self: &Rc<Self>, row: &ShelfRow) {
        match &row.target {
            ShelfTarget::Folder { id, open } => self.set_folder(id, !open),
            ShelfTarget::Playlist(playlist) => self.open_shelf(Source::Playlist(playlist.clone())),
        }
    }

    pub(super) fn set_folder(&self, id: &str, open: bool) {
        self.open_folders.update(|folders| {
            if open {
                folders.insert(id.to_owned());
            } else {
                folders.remove(id);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{ShelfTarget, shelf_rows};
    use crate::icons::PLAIN;
    use crate::spotify::{LibraryEntry, LibraryItem, Playlist};
    use std::{collections::HashSet, rc::Rc};

    fn playlist(uri: &str, folders: &[&str]) -> LibraryEntry {
        LibraryEntry {
            item: LibraryItem::Playlist(Rc::new(Playlist {
                uri: uri.into(),
                name: uri.into(),
            })),
            folders: folders.iter().map(|id| (*id).to_owned()).collect(),
        }
    }

    fn folder(id: &str, folders: &[&str]) -> LibraryEntry {
        LibraryEntry {
            item: LibraryItem::Folder {
                id: id.into(),
                name: id.into(),
            },
            folders: folders.iter().map(|id| (*id).to_owned()).collect(),
        }
    }

    fn library() -> Vec<LibraryEntry> {
        vec![
            playlist("a", &[]),
            folder("outer", &[]),
            playlist("b", &["outer"]),
            folder("inner", &["outer"]),
            playlist("c", &["outer", "inner"]),
            playlist("d", &[]),
        ]
    }

    #[test]
    fn closed_folders_hide_their_contents() {
        let labels = |open: &[&str]| {
            let open: HashSet<String> = open.iter().map(|id| (*id).to_owned()).collect();
            shelf_rows(&library(), &open, None, &PLAIN)
                .into_iter()
                .map(|row| row.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(labels(&[]), ["a", "▸ outer", "d"]);
        assert_eq!(
            labels(&["outer"]),
            ["a", "▾ outer", "  b", "  ▸ inner", "d"]
        );
        assert_eq!(
            labels(&["outer", "inner"]),
            ["a", "▾ outer", "  b", "  ▾ inner", "    c", "d"]
        );
        // An open folder inside a closed one stays hidden.
        assert_eq!(labels(&["inner"]), ["a", "▸ outer", "d"]);
    }

    #[test]
    fn a_closed_folder_shows_the_playing_list_inside() {
        let closed = shelf_rows(&library(), &HashSet::new(), Some("c"), &PLAIN);
        let current: Vec<_> = closed
            .iter()
            .filter(|row| row.current)
            .map(|row| row.key.as_str())
            .collect();
        assert_eq!(current, ["folder:outer"]);
        let open: HashSet<String> = ["outer".to_owned(), "inner".to_owned()].into();
        let opened = shelf_rows(&library(), &open, Some("c"), &PLAIN);
        let current: Vec<_> = opened
            .iter()
            .filter(|row| row.current)
            .map(|row| row.key.as_str())
            .collect();
        assert_eq!(current, ["c"]);
        let inner = opened
            .iter()
            .find(|row| row.key == "c")
            .map(|row| row.parent.clone());
        assert_eq!(inner, Some(Some("inner".to_owned())));
        assert!(matches!(
            opened[1].target,
            ShelfTarget::Folder { open: true, .. }
        ));
    }
}
