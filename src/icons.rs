//! The glyphs of the interface: Nerd Font icons when `SPT_NERD_FONT` is set, plain
//! Unicode otherwise, since a Nerd Font cannot be detected from the terminal.
use std::sync::OnceLock;

pub(crate) struct Icons {
    pub(crate) previous: &'static str,
    pub(crate) play: &'static str,
    pub(crate) pause: &'static str,
    pub(crate) next: &'static str,
    pub(crate) shuffle: &'static str,
    pub(crate) repeat: &'static str,
    pub(crate) repeat_one: &'static str,
    /// From silent to loud; the volume label picks one by level.
    pub(crate) volume: [&'static str; 4],
    pub(crate) search: &'static str,
    pub(crate) liked: &'static str,
    pub(crate) recent: &'static str,
    pub(crate) playlist: &'static str,
    pub(crate) folder_open: &'static str,
    pub(crate) folder_closed: &'static str,
}

/// Material Design icons of Nerd Fonts, one cell wide in the "Mono" variants.
pub(crate) const NERD: Icons = Icons {
    previous: "\u{F04AE}",
    play: "\u{F040A}",
    pause: "\u{F03E4}",
    next: "\u{F04AD}",
    shuffle: "\u{F049D}",
    repeat: "\u{F0456}",
    repeat_one: "\u{F0458}",
    volume: ["\u{F0581}", "\u{F057F}", "\u{F0580}", "\u{F057E}"],
    search: "\u{F0349} ",
    liked: "\u{F02D1} ",
    recent: "\u{F02DA} ",
    playlist: "\u{F0CB8} ",
    folder_open: "\u{F0770} ",
    folder_closed: "\u{F024B} ",
};

pub(crate) const PLAIN: Icons = Icons {
    previous: "|◀",
    play: "▶ Play",
    pause: "‖ Pause",
    next: "▶|",
    shuffle: "⇄ Shuffle",
    repeat: "↻ Repeat",
    repeat_one: "↻ Repeat one",
    volume: [""; 4],
    search: "⌕ ",
    liked: "♥ ",
    recent: "◷ ",
    playlist: "",
    folder_open: "▾ ",
    folder_closed: "▸ ",
};

pub(crate) fn get() -> &'static Icons {
    static ICONS: OnceLock<&'static Icons> = OnceLock::new();
    ICONS.get_or_init(|| {
        let nerd =
            std::env::var("SPT_NERD_FONT").is_ok_and(|value| !value.is_empty() && value != "0");
        if nerd { &NERD } else { &PLAIN }
    })
}
