//! The settings in `$XDG_CONFIG_HOME/spt/config.toml`. The file and each of its
//! settings are optional: what is left out keeps Spotify's look.
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    pub(crate) theme: Theme,
    pub(crate) layout: Layout,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Layout {
    pub(crate) player: PlayerPosition,
    pub(crate) borders: Borders,
}

/// Where the player bar sits, above or below the library.
#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PlayerPosition {
    Top,
    #[default]
    Bottom,
}

/// The corners of the boxes. A terminal draws a corner in one cell, so there is
/// no radius in between.
#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Borders {
    #[default]
    Rounded,
    Square,
}

/// The colors of the interface, each named for what it paints.
#[derive(Clone, Copy, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Theme {
    pub(crate) background: Paint,
    /// Titles, the playing track's name, the focused row.
    pub(crate) text: Paint,
    /// Artists, albums, lengths, the sidebar's rows.
    pub(crate) subtext: Paint,
    /// Column headings, section titles, notes.
    pub(crate) muted: Paint,
    /// Tracks Spotify cannot play, and the player's border while paused.
    pub(crate) unavailable: Paint,
    /// The selected row, and the borders of the boxes without the focus.
    pub(crate) surface: Paint,
    /// What is left of the progress bar, and the search field's border.
    pub(crate) bar: Paint,
    /// What is playing, what has the focus, what is on.
    pub(crate) accent: Paint,
    pub(crate) error: Paint,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Paint::Rgb(0x12, 0x12, 0x12),
            text: Paint::Rgb(0xFF, 0xFF, 0xFF),
            subtext: Paint::Rgb(0xB3, 0xB3, 0xB3),
            muted: Paint::Rgb(0x6A, 0x6A, 0x6A),
            unavailable: Paint::Rgb(0x53, 0x53, 0x53),
            surface: Paint::Rgb(0x2A, 0x2A, 0x2A),
            bar: Paint::Rgb(0x3E, 0x3E, 0x3E),
            accent: Paint::Rgb(0x1D, 0xB9, 0x54),
            error: Paint::Rgb(0xF1, 0x5E, 0x6C),
        }
    }
}

/// A color: `#rrggbb`, or `default` for the terminal's own.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(try_from = "String")]
pub(crate) enum Paint {
    Rgb(u8, u8, u8),
    Default,
}

impl TryFrom<String> for Paint {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        if text == "default" {
            return Ok(Self::Default);
        }
        let invalid = || format!("invalid color \"{text}\": expected #rrggbb or default");
        let hex = text
            .strip_prefix('#')
            .filter(|hex| hex.len() == 6 && hex.chars().all(|digit| digit.is_ascii_hexdigit()))
            .ok_or_else(invalid)?;
        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).map_err(|_| invalid());
        Ok(Self::Rgb(channel(0)?, channel(2)?, channel(4)?))
    }
}

/// Reads the settings; a missing file is no error, a wrong one is.
pub(crate) fn load() -> Result<Config, String> {
    let Some(path) = xdg_dir("XDG_CONFIG_HOME", ".config").map(|dir| dir.join("config.toml"))
    else {
        return Ok(Config::default());
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

/// This program's folder in an XDG base directory, or in its usual place in the home.
pub(crate) fn xdg_dir(variable: &str, fallback: &str) -> Option<PathBuf> {
    let base = std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::home_dir().map(|home| home.join(fallback)))?;
    Some(base.join(crate::COMMAND))
}

#[cfg(test)]
mod tests {
    use super::{Borders, Config, Paint, PlayerPosition};

    #[test]
    fn reads_every_setting() {
        let config: Config = toml::from_str(
            "[layout]\nplayer = \"top\"\nborders = \"square\"\n\
             [theme]\naccent = \"#cba6f7\"\nbackground = \"default\"\n",
        )
        .unwrap();
        assert!(config.layout.player == PlayerPosition::Top);
        assert!(config.layout.borders == Borders::Square);
        assert_eq!(config.theme.accent, Paint::Rgb(0xCB, 0xA6, 0xF7));
        assert_eq!(config.theme.background, Paint::Default);
        // What is left out keeps its default.
        assert_eq!(config.theme.text, Paint::Rgb(0xFF, 0xFF, 0xFF));
    }

    #[test]
    fn rejects_mistakes() {
        for text in [
            "[theme]\naccent = \"green\"",
            "[theme]\naccent = \"#12345\"",
            "[theme]\nacent = \"#123456\"",
            "[layout]\nplayer = \"left\"",
        ] {
            assert!(toml::from_str::<Config>(text).is_err(), "{text}");
        }
    }
}
