//! The colors, corners and sizes of the settings, applied over `ui/terminal.css` when
//! the player starts, so that the settings change them without a new build.
//! `ui/terminal.css` holds no color but the covers', which are image IDs.
use crate::config::{Borders, Config, Paint, Theme};
use hypercmd::style::{
    BorderStyle, Color, Declaration, Length, Rule, Selector,
    Selector::{Class, Focus, Id, Tag},
    StyleSheet,
};

#[derive(Clone, Copy)]
enum Role {
    Background,
    Text,
    Subtext,
    Muted,
    Unavailable,
    Surface,
    Bar,
    Accent,
    Error,
}

#[derive(Clone, Copy)]
enum Property {
    Color,
    Background,
    Border,
}

use Property::{Background as Bg, Border, Color as Fg};

/// A rule's selector, and the colors it gives by role.
type Painted = (&'static [Selector], &'static [(Property, Role)]);

/// The rules of `ui/terminal.css`, in its order, with their colors.
const PAINTED: &[Painted] = &[
    (&[Tag("main")], &[(Fg, Role::Text), (Bg, Role::Background)]),
    (
        &[Tag("button"), Focus],
        &[(Fg, Role::Text), (Bg, Role::Surface)],
    ),
    (&[Class("error")], &[(Fg, Role::Error)]),
    (&[Class("placeholder")], &[(Fg, Role::Muted)]),
    (&[Class("welcome-logo")], &[(Fg, Role::Accent)]),
    (&[Class("welcome-title")], &[(Fg, Role::Text)]),
    (&[Class("welcome-tagline")], &[(Fg, Role::Subtext)]),
    (&[Class("welcome-hint")], &[(Fg, Role::Subtext)]),
    (&[Class("welcome-note")], &[(Fg, Role::Muted)]),
    (
        &[Class("primary")],
        &[(Fg, Role::Background), (Bg, Role::Accent)],
    ),
    (
        &[Class("primary"), Focus],
        &[(Fg, Role::Background), (Bg, Role::Accent)],
    ),
    (&[Class("shelves")], &[(Border, Role::Surface)]),
    // hypercmd has no :focus-within; the shelves report their focus instead.
    (
        &[Class("shelves"), Class("active")],
        &[(Border, Role::Accent)],
    ),
    (&[Class("pane-title")], &[(Fg, Role::Muted)]),
    (&[Class("shelf")], &[(Fg, Role::Subtext)]),
    (
        &[Class("shelf"), Focus],
        &[(Fg, Role::Text), (Bg, Role::Surface)],
    ),
    (&[Class("folder")], &[(Fg, Role::Text)]),
    (&[Class("current")], &[(Fg, Role::Accent)]),
    (&[Class("listing-summary")], &[(Fg, Role::Subtext)]),
    (&[Class("header-kind")], &[(Fg, Role::Subtext)]),
    (&[Class("header-detail")], &[(Fg, Role::Subtext)]),
    (&[Class("header-byline")], &[(Fg, Role::Subtext)]),
    (&[Tag("input")], &[(Border, Role::Bar)]),
    (&[Tag("input"), Focus], &[(Border, Role::Accent)]),
    (&[Id("tracks")], &[(Border, Role::Surface)]),
    (&[Id("tracks"), Focus], &[(Border, Role::Accent)]),
    (&[Class("scroll-track")], &[(Fg, Role::Surface)]),
    (&[Class("scroll-thumb")], &[(Fg, Role::Subtext)]),
    (&[Tag("th")], &[(Fg, Role::Muted)]),
    (&[Class("number")], &[(Fg, Role::Muted)]),
    (&[Class("artist")], &[(Fg, Role::Subtext)]),
    (&[Class("album")], &[(Fg, Role::Subtext)]),
    (&[Class("length")], &[(Fg, Role::Subtext)]),
    (&[Class("selected")], &[(Bg, Role::Surface)]),
    (&[Class("playing")], &[(Fg, Role::Accent)]),
    // Every cell of a track Spotify cannot play here, as Spotify's apps gray them.
    (&[Class("unavailable")], &[(Fg, Role::Unavailable)]),
    (&[Class("empty")], &[(Fg, Role::Subtext)]),
    (&[Class("results-box")], &[(Border, Role::Surface)]),
    (
        &[Class("results-box"), Class("active")],
        &[(Border, Role::Accent)],
    ),
    (&[Class("results-title")], &[(Fg, Role::Muted)]),
    (&[Class("result")], &[(Fg, Role::Subtext)]),
    (
        &[Class("result"), Focus],
        &[(Fg, Role::Text), (Bg, Role::Surface)],
    ),
    (&[Class("results-note")], &[(Fg, Role::Muted)]),
    (&[Class("now-playing")], &[(Border, Role::Accent)]),
    (&[Class("control")], &[(Fg, Role::Subtext)]),
    (&[Class("play")], &[(Fg, Role::Text)]),
    (&[Class("on")], &[(Fg, Role::Accent)]),
    (&[Class("time")], &[(Fg, Role::Subtext)]),
    (&[Class("played")], &[(Fg, Role::Accent)]),
    (&[Class("left")], &[(Fg, Role::Bar)]),
    (&[Class("volume-icon")], &[(Fg, Role::Subtext)]),
    (&[Class("notice")], &[(Fg, Role::Accent)]),
    (&[Class("dialog")], &[(Border, Role::Accent)]),
    (&[Class("dialog-hint")], &[(Fg, Role::Subtext)]),
    (&[Class("keys")], &[(Fg, Role::Text)]),
];

/// The player's border, from the accent to gray in eighths as music pauses: see `run_frames`.
const PAUSED: [&str; 8] = [
    "paused1", "paused2", "paused3", "paused4", "paused5", "paused6", "paused7", "paused8",
];
/// The edge cell of the progress bar, from gray to the accent in eighths.
const EDGE: [&str; 7] = [
    "edge1", "edge2", "edge3", "edge4", "edge5", "edge6", "edge7",
];

/// The rules with a border in `ui/terminal.css`, whose corners the settings choose.
const BORDERED: &[&[Selector]] = &[
    &[Class("shelves")],
    &[Tag("input")],
    &[Id("tracks")],
    &[Class("results-box")],
    &[Class("now-playing")],
    &[Class("dialog")],
];

/// The rules the settings lay over the stylesheet. It lives as long as the
/// program, which builds it once.
pub(crate) fn stylesheet(config: &Config) -> StyleSheet {
    let theme = &config.theme;
    let mut rules: Vec<Rule> = PAINTED
        .iter()
        .map(|&(selector, paints)| Rule {
            selector,
            declarations: leak(
                paints
                    .iter()
                    .map(|(property, role)| declaration(*property, paint(theme, *role)))
                    .collect(),
            ),
        })
        .collect();
    // After `.now-playing` and `.left`, which they override.
    let paused = PAUSED.into_iter().zip(1..).map(|(class, level)| {
        let color = mix(theme.accent, theme.unavailable, level, PAUSED.len());
        (class, declaration(Border, color))
    });
    let edges = EDGE.into_iter().zip(1..).map(|(class, level)| {
        let color = mix(theme.bar, theme.accent, level, EDGE.len() + 1);
        (class, declaration(Fg, color))
    });
    rules.extend(paused.chain(edges).map(|(class, declaration)| Rule {
        selector: leak(vec![Class(class)]),
        declarations: leak(vec![declaration]),
    }));
    let sizes = [
        (
            "shelves",
            Declaration::Width(Length::Cells(f32::from(config.layout.sidebar_width))),
        ),
        (
            "results",
            Declaration::Height(Length::Cells(f32::from(config.layout.results_height))),
        ),
    ];
    rules.extend(sizes.map(|(class, size)| Rule {
        selector: leak(vec![Class(class)]),
        declarations: leak(vec![size]),
    }));
    if config.layout.borders == Borders::Square {
        rules.extend(BORDERED.iter().map(|&selector| Rule {
            selector,
            declarations: &[Declaration::BorderStyle(BorderStyle::Solid)],
        }));
    }
    leak(rules)
}

fn leak<T>(items: Vec<T>) -> &'static [T] {
    Box::leak(items.into_boxed_slice())
}

const fn paint(theme: &Theme, role: Role) -> Paint {
    match role {
        Role::Background => theme.background,
        Role::Text => theme.text,
        Role::Subtext => theme.subtext,
        Role::Muted => theme.muted,
        Role::Unavailable => theme.unavailable,
        Role::Surface => theme.surface,
        Role::Bar => theme.bar,
        Role::Accent => theme.accent,
        Role::Error => theme.error,
    }
}

fn declaration(property: Property, paint: Paint) -> Declaration {
    let color = match paint {
        Paint::Rgb(red, green, blue) => Color::Rgb(red, green, blue),
        Paint::Default => Color::Reset,
    };
    match property {
        Property::Color => Declaration::Foreground(color),
        Property::Background => Declaration::Background(color),
        Property::Border => Declaration::BorderColor(color),
    }
}

/// The color `step` steps of `steps` from `from` to `to`. The terminal's own
/// colors are unknown, so a fade from or to them jumps halfway.
fn mix(from: Paint, to: Paint, step: usize, steps: usize) -> Paint {
    match (from, to) {
        (Paint::Rgb(r1, g1, b1), Paint::Rgb(r2, g2, b2)) => {
            let channel = |start: u8, end: u8| {
                let (start, end) = (f64::from(start), f64::from(end));
                #[expect(
                    clippy::cast_precision_loss,
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a handful of steps between two channels of 0 to 255"
                )]
                let value = (start + (end - start) * step as f64 / steps as f64).round() as u8;
                value
            };
            Paint::Rgb(channel(r1, r2), channel(g1, g2), channel(b1, b2))
        }
        _ if step * 2 < steps => from,
        _ => to,
    }
}

#[cfg(test)]
mod tests {
    use super::{BORDERED, Paint, mix};
    use hypercmd::style::Selector;

    const CSS: &str = include_str!("../ui/terminal.css");

    fn css_selector(selector: &[Selector]) -> String {
        selector
            .iter()
            .map(|part| match part {
                Selector::Tag(tag) => (*tag).to_owned(),
                Selector::Class(class) => format!(".{class}"),
                Selector::Id(id) => format!("#{id}"),
                Selector::Focus => ":focus".to_owned(),
                Selector::Disabled => ":disabled".to_owned(),
            })
            .collect()
    }

    /// The selectors of the stylesheet's rules that hold `property`.
    fn holding(property: &str) -> Vec<String> {
        CSS.split('}')
            .filter_map(|rule| rule.split_once('{'))
            .filter(|(_, body)| {
                body.split(';')
                    .any(|line| line.trim().starts_with(&format!("{property}:")))
            })
            .flat_map(|(selectors, _)| {
                let selectors = selectors.rsplit("*/").next().unwrap_or(selectors);
                selectors
                    .split(',')
                    .map(|selector| selector.trim().to_owned())
            })
            .collect()
    }

    #[test]
    fn colors_come_only_from_the_theme() {
        let colored: Vec<_> = ["color", "background-color", "border-color"]
            .iter()
            .flat_map(|property| holding(property))
            .collect();
        assert_eq!(colored, [".header-cover-row", ".cover-row"]);
    }

    #[test]
    fn every_border_can_turn_square() {
        let bordered: Vec<_> = BORDERED
            .iter()
            .map(|selector| css_selector(selector))
            .collect();
        assert_eq!(holding("border-style"), bordered);
    }

    #[test]
    fn fades_keep_the_original_colors() {
        let green = Paint::Rgb(0x1D, 0xB9, 0x54);
        let gray = Paint::Rgb(0x53, 0x53, 0x53);
        assert_eq!(mix(green, gray, 1, 8), Paint::Rgb(0x24, 0xAC, 0x54));
        assert_eq!(mix(green, gray, 8, 8), gray);
        let left = Paint::Rgb(0x3E, 0x3E, 0x3E);
        assert_eq!(mix(left, green, 1, 8), Paint::Rgb(0x3A, 0x4D, 0x41));
    }
}
