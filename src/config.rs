//! The settings in `$XDG_CONFIG_HOME/spt/config.toml`. The file and each of its
//! settings are optional: what is left out keeps its default.
mod keys;

pub(crate) use keys::{Action, Keys, label as key_label};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

static SETTINGS: OnceLock<Config> = OnceLock::new();

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    pub(crate) theme: Theme,
    pub(crate) layout: Layout,
    pub(crate) interface: Interface,
    pub(crate) playback: Playback,
    pub(crate) cache: Cache,
    pub(crate) search: Search,
    pub(crate) keys: Keys,
    pub(crate) system: System,
}

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Layout {
    pub(crate) player: PlayerPosition,
    pub(crate) borders: Borders,
    /// Whether the sidebar shows at start; a key shows or hides it.
    pub(crate) sidebar: bool,
    pub(crate) sidebar_width: u16,
    /// Rows of the boxes below the songs of a search or an artist.
    pub(crate) results_height: u16,
    pub(crate) columns: Vec<Column>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            player: PlayerPosition::default(),
            borders: Borders::default(),
            sidebar: true,
            sidebar_width: 28,
            results_height: 10,
            columns: Column::ALL.to_vec(),
        }
    }
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

/// A column of the track list.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Column {
    Number,
    Title,
    Artist,
    Album,
    Length,
}

impl Column {
    const ALL: [Self; 5] = [
        Self::Number,
        Self::Title,
        Self::Artist,
        Self::Album,
        Self::Length,
    ];
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

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Interface {
    pub(crate) start: Start,
    /// Whether the selected row comes back with the view at the next start.
    pub(crate) remember_row: bool,
    /// Album covers, in terminals that show images.
    pub(crate) covers: bool,
    /// Nerd Font icons; unset, `SPT_NERD_FONT` decides.
    pub(crate) nerd_font: Option<bool>,
    pub(crate) notice_seconds: u64,
    pub(crate) volume_seconds: u64,
    /// Frames per second of the progress bar while music plays.
    pub(crate) fps: u64,
    /// Whether the player's border fades to gray on pause, rather than at once.
    pub(crate) pause_fade: bool,
    /// The character the progress and volume bars are drawn with.
    pub(crate) bar_glyph: char,
}

impl Default for Interface {
    fn default() -> Self {
        Self {
            start: Start::default(),
            remember_row: true,
            covers: true,
            nerd_font: None,
            notice_seconds: 4,
            volume_seconds: 2,
            fps: 60,
            pause_fade: true,
            bar_glyph: '━',
        }
    }
}

/// What the player opens when it starts.
#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Start {
    /// The view shown when it last closed.
    #[default]
    Last,
    Liked,
    Recent,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Playback {
    /// kbit/s: 96, 160 or 320.
    pub(crate) bitrate: u16,
    /// Evens out the loudness of tracks, as Spotify's apps do by default.
    pub(crate) normalisation: bool,
    pub(crate) normalisation_type: NormalisationType,
    /// dB added before normalising.
    pub(crate) normalisation_pregain: f64,
    pub(crate) gapless: bool,
    pub(crate) autoplay: Autoplay,
    /// Percent. Full by default: the system mixer sets the loudness, and librespot's
    /// curve is logarithmic over 60 dB, so half of the range already plays 30 dB quieter.
    pub(crate) initial_volume: u8,
    /// Percent of the volume keys.
    pub(crate) volume_step: u8,
    pub(crate) seek_seconds: u64,
    /// The arrows' step in a podcast episode.
    pub(crate) podcast_seek_seconds: u64,
    /// The fade of the sound around pause and resume; 0 cuts at once.
    pub(crate) fade_ms: u64,
    /// The audio output by name, as `spt devices` lists them; empty for the system's.
    pub(crate) output: String,
    /// The name other Spotify apps list this computer as.
    pub(crate) device_name: String,
    /// The icon other Spotify apps show for it.
    pub(crate) device_type: String,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            bitrate: 320,
            normalisation: true,
            normalisation_type: NormalisationType::default(),
            normalisation_pregain: 0.0,
            gapless: true,
            autoplay: Autoplay::default(),
            initial_volume: 100,
            volume_step: 5,
            seek_seconds: 10,
            podcast_seek_seconds: 30,
            fade_ms: 150,
            output: String::new(),
            device_name: "Spotify".into(),
            device_type: "computer".into(),
        }
    }
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum NormalisationType {
    /// Albums as they were mastered when one plays in order, tracks otherwise.
    #[default]
    Auto,
    Album,
    Track,
}

/// Whether similar tracks play on once a list ends.
#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Autoplay {
    /// As the account's setting in Spotify's apps.
    #[default]
    Account,
    On,
    Off,
}

/// Tracks played before, kept on disk so they play again without downloading.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Cache {
    pub(crate) enabled: bool,
    pub(crate) size_mb: u64,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            enabled: true,
            size_mb: 1024,
        }
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Search {
    /// How long typing pauses before a search of the library follows it.
    pub(crate) library_delay_ms: u64,
    /// The same for the catalog, which asks Spotify each time.
    pub(crate) catalog_delay_ms: u64,
    /// Results in each box.
    pub(crate) results: usize,
    /// The boxes of a catalog search, in their order.
    pub(crate) boxes: Vec<SearchBox>,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            library_delay_ms: 80,
            catalog_delay_ms: 300,
            results: 8,
            boxes: SearchBox::ALL.to_vec(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SearchBox {
    Artists,
    Albums,
    Playlists,
    Podcasts,
}

impl SearchBox {
    const ALL: [Self; 4] = [Self::Artists, Self::Albums, Self::Playlists, Self::Podcasts];

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Artists => "Artists",
            Self::Albums => "Albums",
            Self::Playlists => "Playlists",
            Self::Podcasts => "Podcasts",
        }
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct System {
    /// Media keys, status bars and `playerctl`, on Linux.
    pub(crate) mpris: bool,
    pub(crate) reconnect_seconds: u64,
    /// The command that opens the login page.
    pub(crate) browser: String,
}

impl Default for System {
    fn default() -> Self {
        Self {
            mpris: true,
            reconnect_seconds: 5,
            browser: if cfg!(target_os = "macos") {
                "open"
            } else {
                "xdg-open"
            }
            .into(),
        }
    }
}

impl Config {
    /// Checks what types alone cannot.
    fn validate(&self) -> Result<(), String> {
        let failed = self
            .layout_checks()
            .into_iter()
            .chain(self.playback_checks())
            .find(|(valid, _)| !valid);
        if let Some((_, message)) = failed {
            return Err(message.to_owned());
        }
        self.keys.validate()
    }

    fn layout_checks(&self) -> [(bool, &'static str); 8] {
        let (layout, interface) = (&self.layout, &self.interface);
        [
            (
                (12..=80).contains(&layout.sidebar_width),
                "layout.sidebar_width: between 12 and 80",
            ),
            (
                (4..=40).contains(&layout.results_height),
                "layout.results_height: between 4 and 40",
            ),
            (
                layout.columns.contains(&Column::Title),
                "layout.columns: the title cannot be left out",
            ),
            (
                (1..=120).contains(&interface.fps),
                "interface.fps: between 1 and 120",
            ),
            (
                interface.notice_seconds >= 1,
                "interface.notice_seconds: at least 1",
            ),
            (
                (1..=50).contains(&self.search.results),
                "search.results: between 1 and 50",
            ),
            (
                !self.system.browser.trim().is_empty(),
                "system.browser: cannot be empty",
            ),
            (
                self.keys.gg_delay_ms >= 100,
                "keys.gg_delay_ms: at least 100",
            ),
        ]
    }

    fn playback_checks(&self) -> [(bool, &'static str); 8] {
        let playback = &self.playback;
        [
            (
                matches!(playback.bitrate, 96 | 160 | 320),
                "playback.bitrate: 96, 160 or 320",
            ),
            (
                (-20.0..=20.0).contains(&playback.normalisation_pregain),
                "playback.normalisation_pregain: between -20 and 20 dB",
            ),
            (
                playback.initial_volume <= 100,
                "playback.initial_volume: at most 100",
            ),
            (
                (1..=50).contains(&playback.volume_step),
                "playback.volume_step: between 1 and 50",
            ),
            (
                playback.seek_seconds >= 1 && playback.podcast_seek_seconds >= 1,
                "playback.seek_seconds and podcast_seek_seconds: at least 1",
            ),
            (playback.fade_ms <= 2000, "playback.fade_ms: at most 2000"),
            (
                !playback.device_name.trim().is_empty(),
                "playback.device_name: cannot be empty",
            ),
            (
                playback
                    .device_type
                    .parse::<librespot_core::config::DeviceType>()
                    .is_ok(),
                "playback.device_type: computer, speaker, tv, smartphone, tablet, avr, stb, \
                 audiodongle, gameconsole, castaudio, automobile, smartwatch or chromebook",
            ),
        ]
    }

    pub(crate) fn notice_ticks(&self) -> usize {
        ticks(self.interface.notice_seconds)
    }

    pub(crate) fn volume_ticks(&self) -> usize {
        ticks(self.interface.volume_seconds)
    }

    pub(crate) fn frame(&self) -> Duration {
        Duration::from_secs(1) / u32::try_from(self.interface.fps).unwrap_or(60)
    }

    /// The speaker's volume of `initial_volume`.
    pub(crate) fn initial_volume(&self) -> u16 {
        volume(self.playback.initial_volume)
    }

    pub(crate) fn volume_step(&self) -> u16 {
        volume(self.playback.volume_step)
    }

    pub(crate) fn seek_step(&self, episode: bool) -> Duration {
        Duration::from_secs(if episode {
            self.playback.podcast_seek_seconds
        } else {
            self.playback.seek_seconds
        })
    }
}

/// Clock ticks in `seconds`; the clock ticks four times a second.
fn ticks(seconds: u64) -> usize {
    usize::try_from(seconds.saturating_mul(4)).unwrap_or(usize::MAX)
}

/// The speaker's volume, out of `u16::MAX`, at `percent`.
fn volume(percent: u8) -> u16 {
    let value = u32::from(u16::MAX) * u32::from(percent.min(100)) / 100;
    u16::try_from(value).unwrap_or(u16::MAX)
}

/// The settings the program started with, or the defaults before that.
pub(crate) fn get() -> &'static Config {
    SETTINGS.get_or_init(Config::default)
}

/// Makes `config` the settings of the whole program; only the first call counts.
pub(crate) fn install(config: Config) {
    let _ = SETTINGS.set(config);
}

/// The default settings file.
pub(crate) fn default_path() -> Option<PathBuf> {
    xdg_dir("XDG_CONFIG_HOME", ".config").map(|dir| dir.join("config.toml"))
}

/// Reads the settings at `path`, or the default file; a missing default file is
/// no error, a wrong file or a missing given one is.
pub(crate) fn load(path: Option<&Path>) -> Result<Config, String> {
    let (path, given) = match path {
        Some(path) => (path.to_path_buf(), true),
        None => match default_path() {
            Some(path) => (path, false),
            None => return Ok(Config::default()),
        },
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => parse(&text).map_err(|error| format!("{}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !given => {
            Ok(Config::default())
        }
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn parse(text: &str) -> Result<Config, String> {
    let config: Config = toml::from_str(text).map_err(|error| error.to_string())?;
    config.validate()?;
    Ok(config)
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
    use super::{Autoplay, Borders, Column, Config, Paint, PlayerPosition, SearchBox, parse};

    #[test]
    fn reads_every_section() {
        let config = parse(
            "[layout]\nplayer = \"top\"\nborders = \"square\"\ncolumns = [\"title\", \"artist\"]\n\
             [theme]\naccent = \"#cba6f7\"\nbackground = \"default\"\n\
             [playback]\nbitrate = 160\nautoplay = \"off\"\n\
             [search]\nboxes = [\"artists\"]\n\
             [keys]\nnext = [\"N\"]\n",
        )
        .unwrap();
        assert!(config.layout.player == PlayerPosition::Top);
        assert!(config.layout.borders == Borders::Square);
        assert_eq!(config.layout.columns, [Column::Title, Column::Artist]);
        assert_eq!(config.theme.accent, Paint::Rgb(0xCB, 0xA6, 0xF7));
        assert_eq!(config.theme.background, Paint::Default);
        assert_eq!(config.playback.bitrate, 160);
        assert!(config.playback.autoplay == Autoplay::Off);
        assert_eq!(config.search.boxes, [SearchBox::Artists]);
        // What is left out keeps its default.
        assert_eq!(config.theme.text, Paint::Rgb(0xFF, 0xFF, 0xFF));
        assert_eq!(config.playback.podcast_seek_seconds, 30);
    }

    #[test]
    fn rejects_mistakes() {
        for text in [
            "[theme]\naccent = \"green\"",
            "[theme]\naccent = \"#12345\"",
            "[theme]\nacent = \"#123456\"",
            "[layout]\nplayer = \"left\"",
            "[layout]\ncolumns = [\"artist\"]",
            "[layout]\nsidebar_width = 4",
            "[playback]\nbitrate = 128",
            "[playback]\ndevice_type = \"toaster\"",
            "[playback]\ninitial_volume = 120",
            "[interface]\nfps = 0",
            "[search]\nresults = 0",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn the_defaults_are_valid() {
        assert!(Config::default().validate().is_ok());
    }

    #[test]
    fn volumes_cover_the_whole_range() {
        assert_eq!(super::volume(100), u16::MAX);
        assert_eq!(super::volume(0), 0);
        assert_eq!(super::volume(5), u16::MAX / 20);
    }
}
