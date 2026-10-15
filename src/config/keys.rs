//! The keys of the `[keys]` section: each action and the keys that do it.
use hypercmd::Key;
use serde::Deserialize;

/// What a single-key shortcut does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Action {
    PlayPause,
    Next,
    Previous,
    SeekForward,
    SeekBack,
    VolumeUp,
    VolumeDown,
    Shuffle,
    Repeat,
    OpenAlbum,
    OpenArtist,
    Queue,
    Search,
    LibrarySearch,
    ToggleSidebar,
    Help,
    Quit,
    Back,
}

/// Keys the lists and the sidebar keep for moving around.
const RESERVED: [Key; 10] = [
    Key::Char('j'),
    Key::Char('k'),
    Key::Char('h'),
    Key::Char('l'),
    Key::Char('g'),
    Key::Char('G'),
    Key::Enter,
    Key::Tab,
    Key::Up,
    Key::Down,
];

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Keys {
    play_pause: Vec<String>,
    next: Vec<String>,
    previous: Vec<String>,
    seek_forward: Vec<String>,
    seek_back: Vec<String>,
    volume_up: Vec<String>,
    volume_down: Vec<String>,
    shuffle: Vec<String>,
    repeat: Vec<String>,
    open_album: Vec<String>,
    open_artist: Vec<String>,
    queue: Vec<String>,
    search: Vec<String>,
    library_search: Vec<String>,
    toggle_sidebar: Vec<String>,
    help: Vec<String>,
    quit: Vec<String>,
    back: Vec<String>,
    /// How long after a first `g` a second one still makes `gg`.
    pub(crate) gg_delay_ms: u64,
}

fn keys(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

impl Default for Keys {
    fn default() -> Self {
        Self {
            play_pause: keys(&["space"]),
            next: keys(&["n"]),
            previous: keys(&["p"]),
            seek_forward: keys(&["right"]),
            seek_back: keys(&["left"]),
            volume_up: keys(&["+", "="]),
            volume_down: keys(&["-"]),
            shuffle: keys(&["s"]),
            repeat: keys(&["r"]),
            open_album: keys(&["o"]),
            open_artist: keys(&["a"]),
            queue: keys(&["z"]),
            search: keys(&["/"]),
            library_search: keys(&["f"]),
            toggle_sidebar: keys(&["b"]),
            help: keys(&["?"]),
            quit: keys(&["q"]),
            back: keys(&["escape", "backspace"]),
            gg_delay_ms: 800,
        }
    }
}

impl Keys {
    /// Every action with its keys as written, in the order the help lists them.
    pub(crate) fn bindings(&self) -> [(Action, &[String], &'static str); 18] {
        [
            (Action::PlayPause, &self.play_pause, "play or pause"),
            (Action::Next, &self.next, "next track"),
            (Action::Previous, &self.previous, "previous track"),
            (Action::SeekForward, &self.seek_forward, "seek forward"),
            (Action::SeekBack, &self.seek_back, "seek back"),
            (Action::VolumeUp, &self.volume_up, "volume up"),
            (Action::VolumeDown, &self.volume_down, "volume down"),
            (Action::Shuffle, &self.shuffle, "shuffle"),
            (Action::Repeat, &self.repeat, "repeat"),
            (Action::OpenAlbum, &self.open_album, "open the row's album"),
            (
                Action::OpenArtist,
                &self.open_artist,
                "open the row's artist",
            ),
            (Action::Queue, &self.queue, "add the row to the queue"),
            (Action::Search, &self.search, "search"),
            (
                Action::LibrarySearch,
                &self.library_search,
                "search your library",
            ),
            (
                Action::ToggleSidebar,
                &self.toggle_sidebar,
                "show or hide the sidebar",
            ),
            (Action::Help, &self.help, "these keys"),
            (Action::Quit, &self.quit, "quit"),
            (Action::Back, &self.back, "go back"),
        ]
    }

    /// The action `key` does, if any.
    pub(crate) fn action(&self, key: Key) -> Option<Action> {
        self.bindings().into_iter().find_map(|(action, names, _)| {
            names
                .iter()
                .any(|name| parse(name) == Some(key))
                .then_some(action)
        })
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        let mut taken: Vec<(Key, &str)> = Vec::new();
        let named = self
            .bindings()
            .into_iter()
            .flat_map(|(_, names, description)| {
                names.iter().map(move |name| (name.as_str(), description))
            });
        for (name, description) in named {
            taken.push((usable(name, description, &taken)?, description));
        }
        Ok(())
    }
}

/// The key `name` names, if it exists, leaves the lists alone and is not `taken`.
fn usable(name: &str, description: &str, taken: &[(Key, &str)]) -> Result<Key, String> {
    let key = parse(name).ok_or_else(|| {
        format!(
            "keys: unknown key \"{name}\": expected one character, or space, enter, tab, \
             backspace, delete, escape, left, right, up, down, home, end, pageup or pagedown"
        )
    })?;
    if RESERVED.contains(&key) {
        return Err(format!(
            "keys: \"{name}\" moves in the lists and cannot do \"{description}\""
        ));
    }
    if let Some((_, other)) = taken.iter().find(|(used, _)| *used == key) {
        return Err(format!(
            "keys: \"{name}\" cannot do both \"{other}\" and \"{description}\""
        ));
    }
    Ok(key)
}

/// A key by its name in the settings: one character, or the name of a special key.
pub(crate) fn parse(name: &str) -> Option<Key> {
    let mut characters = name.chars();
    if let (Some(character), None) = (characters.next(), characters.next()) {
        return Some(Key::Char(character));
    }
    Some(match name.to_ascii_lowercase().as_str() {
        "space" => Key::Char(' '),
        "enter" => Key::Enter,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "escape" | "esc" => Key::Escape,
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        _ => return None,
    })
}

/// How the help writes a key.
pub(crate) fn label(name: &str) -> String {
    match parse(name) {
        Some(Key::Char(' ')) => "Space".into(),
        Some(Key::Left) => "←".into(),
        Some(Key::Right) => "→".into(),
        Some(Key::Escape) => "Esc".into(),
        Some(Key::Backspace) => "⌫".into(),
        _ if name.chars().count() == 1 => name.into(),
        _ => {
            let mut characters = name.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Keys, parse};
    use hypercmd::Key;

    #[test]
    fn the_defaults_are_the_old_keys() {
        let keys = Keys::default();
        assert!(keys.validate().is_ok());
        assert_eq!(keys.action(Key::Char(' ')), Some(Action::PlayPause));
        assert_eq!(keys.action(Key::Char('=')), Some(Action::VolumeUp));
        assert_eq!(keys.action(Key::Backspace), Some(Action::Back));
        assert_eq!(keys.action(Key::Char('x')), None);
    }

    #[test]
    fn names_read_as_keys() {
        assert_eq!(parse("N"), Some(Key::Char('N')));
        assert_eq!(parse("PageDown"), Some(Key::PageDown));
        assert_eq!(parse("ctrl+a"), None);
    }

    #[test]
    fn a_key_does_one_thing_and_leaves_the_lists_alone() {
        let twice = Keys {
            next: vec!["s".into()],
            ..Keys::default()
        };
        assert!(twice.validate().is_err());
        let moving = Keys {
            next: vec!["j".into()],
            ..Keys::default()
        };
        assert!(moving.validate().is_err());
    }
}
