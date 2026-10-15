use super::{
    Jukebox, Screen,
    library::{FIXED_SHELVES, ShelfRow, ShelfTarget},
    playback::VolumeStep,
};
use crate::{
    config::{self, Action},
    spotify::Command,
};
use hypercmd::{Error, ErrorKind, Event, EventPayload, Input, Key, Node};
use std::rc::Rc;

/// What `g` and `G` do in a list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Jump {
    /// `gg`: the first row.
    Top,
    /// `G`: the last row.
    Bottom,
    /// A first `g`, waiting for the second.
    Pending,
}

impl Jukebox {
    /// Reads `g` and `G` as Vim does: `gg` goes to the top and `G` to the bottom.
    pub(super) fn jump(&self, key: Key) -> Option<Jump> {
        match key {
            Key::Char('G') => {
                self.pending_g.set(None);
                Some(Jump::Bottom)
            }
            Key::Char('g') => {
                let earlier = self.pending_g.take();
                let delay = std::time::Duration::from_millis(config::get().keys.gg_delay_ms);
                if earlier.is_some_and(|pressed| pressed.elapsed() < delay) {
                    Some(Jump::Top)
                } else {
                    self.pending_g.set(Some(std::time::Instant::now()));
                    Some(Jump::Pending)
                }
            }
            _ => {
                self.pending_g.set(None);
                None
            }
        }
    }

    pub(super) fn shortcut(self: &Rc<Self>, event: &Event) -> Result<(), Error> {
        let EventPayload::Input(Input::Key { key, modifiers, .. }) = event.payload else {
            return Ok(());
        };
        if self.screen.get_untracked() != Screen::Library {
            return Ok(());
        }
        if self.help.get_untracked() {
            if key == Key::Escape {
                self.close_help();
                event.prevent_default();
            }
            return Ok(());
        }
        if modifiers.control || modifiers.alt || modifiers.super_key {
            if key == Key::Char('f') && modifiers.control {
                self.start_search();
                event.prevent_default();
            }
            return Ok(());
        }
        if event.target.tag() == "input" || !self.single_key(key)? {
            return Ok(());
        }
        event.prevent_default();
        Ok(())
    }

    /// Handles a single-key shortcut of the settings; returns whether `key` was one.
    fn single_key(self: &Rc<Self>, key: Key) -> Result<bool, Error> {
        let Some(action) = config::get().keys.action(key) else {
            return Ok(false);
        };
        match action {
            Action::PlayPause => self.toggle_play()?,
            Action::Next => self.command(Command::Next),
            Action::Previous => self.command(Command::Previous),
            Action::SeekForward => self.seek_forward(),
            Action::SeekBack => self.seek_back(),
            Action::VolumeUp => self.change_volume(VolumeStep::Up),
            Action::VolumeDown => self.change_volume(VolumeStep::Down),
            Action::Shuffle => self.toggle_shuffle(),
            Action::Repeat => self.cycle_repeat(),
            Action::OpenAlbum => self.open_album(),
            Action::OpenArtist => self.open_artist(),
            Action::Queue => self.queue_selected()?,
            Action::Search => self.start_search(),
            Action::LibrarySearch => self.start_library_search(),
            Action::ToggleSidebar => self.toggle_sidebar(),
            Action::Help => self.open_help(),
            Action::Quit => quit()?,
            Action::Back => return Ok(self.back()),
        }
        Ok(true)
    }

    /// Shows or hides the sidebar; hidden, it gives the focus back to the list.
    pub(super) fn toggle_sidebar(&self) {
        let shown = !self.sidebar.get_untracked();
        self.sidebar.set(shown);
        if !shown {
            self.focus("tracks");
        }
    }

    /// The keys of the help, from the settings.
    pub(super) fn help_text(&self) -> String {
        let settings = config::get();
        let mut lines: Vec<(String, &str)> = settings
            .keys
            .bindings()
            .iter()
            .filter(|(_, names, _)| !names.is_empty())
            .map(|(_, names, description)| {
                let keys: Vec<String> = names.iter().map(|name| config::key_label(name)).collect();
                (keys.join(" "), *description)
            })
            .collect();
        lines.extend([
            ("↑ ↓  j k".to_owned(), "choose a row"),
            ("gg / G".to_owned(), "first / last row"),
            ("Enter".to_owned(), "play the row"),
            ("Tab  h l".to_owned(), "sidebar ⇄ list, search boxes"),
            ("^F".to_owned(), "search"),
        ]);
        let width = lines
            .iter()
            .map(|(keys, _)| keys.chars().count())
            .max()
            .unwrap_or(0);
        let column = |(keys, description): &(String, &str)| {
            format!(
                "{keys}{}  {description}",
                " ".repeat(width - keys.chars().count())
            )
        };
        let half = lines.len().div_ceil(2);
        let left_width = lines[..half]
            .iter()
            .map(|line| column(line).chars().count())
            .max()
            .unwrap_or(0);
        (0..half)
            .map(|row| {
                let left = column(&lines[row]);
                match lines.get(row + half) {
                    Some(right) => format!(
                        "{left}{}   {}",
                        " ".repeat(left_width - left.chars().count()),
                        column(right)
                    ),
                    None => left,
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Tab or `l` leaves the sidebar for the track list instead of visiting every
    /// playlist; `j` and `k` move between shelves like the arrow keys.
    pub(super) fn leave_shelves(&self, event: &Event) {
        let EventPayload::Input(Input::Key { key, modifiers, .. }) = event.payload else {
            return;
        };
        if modifiers.control || modifiers.alt || modifiers.super_key {
            return;
        }
        let handled = match (key, self.jump(key)) {
            (_, Some(Jump::Pending)) => true,
            (_, Some(jump)) => {
                let shelves = self.shelves();
                let target = if jump == Jump::Top {
                    shelves.first()
                } else {
                    shelves.last()
                };
                if let Some(shelf) = target {
                    shelf.request_focus();
                }
                true
            }
            (Key::Tab, _) if !modifiers.shift => {
                self.focus("tracks");
                true
            }
            (key, _) => self.shelf_key(key, &event.target),
        };
        if handled {
            event.prevent_default();
        }
    }

    /// The vim-like keys of the sidebar; returns whether `key` was one.
    fn shelf_key(&self, key: Key, target: &Node) -> bool {
        match key {
            Key::Char('l') => self.shelf_right(target),
            Key::Char('h') => self.shelf_left(target),
            Key::Char('j') => self.step_shelf(target, true),
            Key::Char('k') => self.step_shelf(target, false),
            _ => return false,
        }
        true
    }

    /// `h` in the track list goes to the shelf that list comes from, or the first one.
    pub(super) fn enter_shelves(&self) {
        if !self.sidebar.get_untracked() {
            return;
        }
        let shelves = self.shelves();
        let current = shelves.iter().find(|shelf| {
            shelf
                .attribute("class")
                .is_some_and(|classes| classes.split_whitespace().any(|class| class == "current"))
        });
        if let Some(shelf) = current.or(shelves.first()) {
            shelf.request_focus();
        }
    }

    /// The playlist-section row of a sidebar button, if it is one.
    fn shelf_row(&self, button: &Node) -> Option<(Vec<Rc<ShelfRow>>, usize)> {
        let index = self.shelves().iter().position(|shelf| shelf == button)?;
        let rows = fusor::untrack(|| self.shelf_rows());
        let row = index.checked_sub(FIXED_SHELVES)?;
        (row < rows.len()).then_some((rows, row))
    }

    /// `l` opens a closed folder; anywhere else it goes to the track list.
    fn shelf_right(&self, button: &Node) {
        if let Some((rows, index)) = self.shelf_row(button)
            && let ShelfTarget::Folder { id, open: false } = &rows[index].target
        {
            self.set_folder(id, true);
            return;
        }
        self.focus("tracks");
    }

    /// `h` closes an open folder, or goes up to the folder around the row and closes it.
    fn shelf_left(&self, button: &Node) {
        let Some((rows, index)) = self.shelf_row(button) else {
            return;
        };
        if let ShelfTarget::Folder { id, open: true } = &rows[index].target {
            self.set_folder(id, false);
            return;
        }
        let Some(parent) = rows[index].parent.clone() else {
            return;
        };
        let key = format!("folder:{parent}");
        if let Some(position) = rows.iter().position(|row| row.key == key)
            && let Some(folder) = self.shelves().get(FIXED_SHELVES + position)
        {
            folder.request_focus();
        }
        self.set_folder(&parent, false);
    }

    fn step_shelf(&self, from: &Node, down: bool) {
        let shelves = self.shelves();
        let Some(index) = shelves.iter().position(|shelf| shelf == from) else {
            return;
        };
        let next = if down {
            shelves.get(index + 1)
        } else {
            index
                .checked_sub(1)
                .and_then(|previous| shelves.get(previous))
        };
        if let Some(shelf) = next {
            shelf.request_focus();
        }
    }

    fn shelves(&self) -> Vec<Node> {
        self.root
            .borrow()
            .as_ref()
            .and_then(|root| root.find("shelves"))
            .map(|shelves| {
                shelves
                    .descendants()
                    .filter(|node| node.tag() == "button")
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn open_help(&self) {
        self.help.set(true);
    }

    pub(super) fn close_help(&self) {
        self.help.set(false);
        self.focus("tracks");
    }
}

/// Asks the terminal runner for the same orderly shutdown as Ctrl+C.
fn quit() -> Result<(), Error> {
    signal_hook::low_level::raise(signal_hook::consts::SIGTERM)
        .map_err(|error| Error::new(ErrorKind::Terminal, format!("cannot quit: {error}")))
}
