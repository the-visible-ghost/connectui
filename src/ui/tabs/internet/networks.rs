use crate::network::{Security, WifiNetwork};
use crate::ui::common::Theme;
use iocraft::components::{ScrollView, ScrollViewHandle};
use iocraft::hooks::Ref;
use iocraft::prelude::*;

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const MARKER_WIDTH: u32 = 2;
const KNOWN_NAME_PCT: u32 = 32;
const KNOWN_SECURITY_PCT: u32 = 14;
const KNOWN_HIDDEN_PCT: u32 = 12;
const KNOWN_AUTO_CONNECT_PCT: u32 = 19;
const KNOWN_SIGNAL_PCT: u32 = 20;
const NEW_NAME_PCT: u32 = 53;
const NEW_SECURITY_PCT: u32 = 24;
const NEW_SIGNAL_PCT: u32 = 20;

pub fn spinner_frame(index: usize) -> &'static str {
    SPINNER[index % SPINNER.len()]
}

#[derive(Clone)]
pub struct PendingConnect {
    pub ssid: String,
    pub security: Security,
}

pub struct NetworksView<'a> {
    pub theme: Theme,
    pub focused: bool,
    pub networks: &'a [WifiNetwork],
    pub selected: usize,
    pub table_width: u32,
    pub status: Option<&'a str>,
    pub spinner: &'a str,
    pub handle: Ref<ScrollViewHandle>,
}

fn pct(value: u32) -> Percent {
    Percent(value as f32)
}

fn truncate_ssid(ssid: &str, width: u32) -> String {
    let width = width.max(1) as usize;
    if ssid.chars().count() <= width {
        return ssid.to_string();
    }
    let mut value: String = ssid.chars().take(width - 1).collect();
    value.push('…');
    value
}

fn security_color(security: Security) -> Color {
    match security {
        Security::Open => Color::Grey,
        Security::Wep | Security::Wpa => Color::Yellow,
        Security::Wpa2 => Color::Green,
        Security::Wpa3 | Security::Owe => Color::Cyan,
        Security::Enterprise => Color::Magenta,
    }
}

fn signal_color(strength: u8) -> Color {
    match strength {
        75..=100 => Color::Green,
        50..=74 => Color::Cyan,
        25..=49 => Color::Yellow,
        _ => Color::Red,
    }
}

fn signal_icon(strength: u8) -> &'static str {
    match strength {
        75..=100 => "󰤨",
        50..=74 => "󰤥",
        25..=49 => "󰤢",
        _ => "󰤟",
    }
}

pub fn selected_row(networks: &[WifiNetwork], index: usize) -> usize {
    let known_count = networks.iter().filter(|network| network.known).count();
    if index < known_count {
        index + 3
    } else {
        let new_index = index - known_count;
        if known_count > 0 {
            known_count + 8 + new_index
        } else {
            new_index + 3
        }
    }
}

fn text_cell(width: u32, content: String, color: Color, weight: Weight) -> AnyElement<'static> {
    element! {
        View(
            width: pct(width),
            justify_content: JustifyContent::Center,
            overflow: Overflow::Hidden,
        ) {
            Text(
                content: content,
                color: color,
                weight: weight,
                wrap: TextWrap::NoWrap,
            )
        }
    }
    .into_any()
}

fn section_header(label: &str, color: Color) -> AnyElement<'static> {
    element! {
        View(
            width: 100pct,
            padding_left: 1,
            padding_right: 1,
            gap: 1,
            align_items: AlignItems::Center,
        ) {
            Text(
                content: label.to_string(),
                color: color,
                weight: Weight::Bold,
                wrap: TextWrap::NoWrap,
            )
            View(
                flex_grow: 1.0,
                height: 1,
                border_style: BorderStyle::Round,
                border_edges: Edges::Bottom,
                border_color: Color::DarkGrey,
            )
        }
    }
    .into_any()
}

fn table_header(columns: &[(&str, u32)], color: Color) -> AnyElement<'static> {
    let cells: Vec<AnyElement<'static>> = columns
        .iter()
        .map(|(label, width)| text_cell(*width, label.to_string(), color, Weight::Bold))
        .collect();
    element! {
        View(width: 100pct, padding_left: 1, padding_right: 1) {
            View(width: MARKER_WIDTH)
            #(cells)
        }
    }
    .into_any()
}

fn table_separator() -> AnyElement<'static> {
    element! {
        View(
            width: 100pct,
            border_style: BorderStyle::Round,
            border_color: Color::DarkGrey,
            border_edges: Edges::Bottom,
        )
    }
    .into_any()
}

fn spacer() -> AnyElement<'static> {
    element! {
        View(height: 2)
    }
    .into_any()
}

fn name_parts(theme: Theme, network: &WifiNetwork, width: u32) -> (String, Color, Weight) {
    let color = if network.active {
        theme.primary
    } else {
        Color::White
    };
    let weight = if network.active {
        Weight::Bold
    } else {
        Weight::Normal
    };
    (truncate_ssid(&network.ssid, width), color, weight)
}

fn row_background(is_selected: bool) -> Option<Color> {
    is_selected.then_some(Color::DarkGrey)
}

fn known_row(
    theme: Theme,
    table_width: u32,
    network: &WifiNetwork,
    is_selected: bool,
) -> AnyElement<'static> {
    let (name, name_color, name_weight) = name_parts(
        theme,
        network,
        table_width.saturating_mul(KNOWN_NAME_PCT) / 100,
    );
    let hidden = if network.hidden { "Yes" } else { "No" };
    let hidden_color = if network.hidden {
        Color::Yellow
    } else {
        Color::Grey
    };
    let auto_connect = if network.auto_connect { "Yes" } else { "No" };
    let auto_connect_color = if network.auto_connect {
        Color::Green
    } else {
        Color::Grey
    };

    element! {
        View(
            width: 100pct,
            padding_left: 1,
            padding_right: 1,
            background_color: row_background(is_selected),
        ) {
            View(width: MARKER_WIDTH, justify_content: JustifyContent::Center) {
                Text(
                    content: if network.active { "󰤥" } else { "" },
                    color: theme.primary,
                    weight: Weight::Bold,
                )
            }
            #(Some(text_cell(KNOWN_NAME_PCT, name, name_color, name_weight)))
            #(Some(text_cell(
                KNOWN_SECURITY_PCT,
                network.security.label().to_string(),
                security_color(network.security),
                Weight::Normal,
            )))
            #(Some(text_cell(
                KNOWN_HIDDEN_PCT,
                hidden.to_string(),
                hidden_color,
                Weight::Normal,
            )))
            #(Some(text_cell(
                KNOWN_AUTO_CONNECT_PCT,
                auto_connect.to_string(),
                auto_connect_color,
                Weight::Normal,
            )))
            #(Some(text_cell(
                KNOWN_SIGNAL_PCT,
                format!("{}% {}", network.strength, signal_icon(network.strength)),
                signal_color(network.strength),
                Weight::Normal,
            )))
        }
    }
    .into_any()
}

fn new_row(
    theme: Theme,
    table_width: u32,
    network: &WifiNetwork,
    is_selected: bool,
) -> AnyElement<'static> {
    let (name, name_color, name_weight) = name_parts(
        theme,
        network,
        table_width.saturating_mul(NEW_NAME_PCT) / 100,
    );

    element! {
        View(
            width: 100pct,
            padding_left: 1,
            padding_right: 1,
            background_color: row_background(is_selected),
        ) {
            View(width: MARKER_WIDTH, justify_content: JustifyContent::Center) {
                Text(
                    content: if network.active { "󰤥" } else { "" },
                    color: theme.primary,
                    weight: Weight::Bold,
                )
            }
            #(Some(text_cell(NEW_NAME_PCT, name, name_color, name_weight)))
            #(Some(text_cell(
                NEW_SECURITY_PCT,
                network.security.label().to_string(),
                security_color(network.security),
                Weight::Normal,
            )))
            #(Some(text_cell(
                NEW_SIGNAL_PCT,
                format!("{}% {}", network.strength, signal_icon(network.strength)),
                signal_color(network.strength),
                Weight::Normal,
            )))
        }
    }
    .into_any()
}

pub fn networks_panel(view: NetworksView<'_>) -> AnyElement<'static> {
    let count = view.networks.len();
    let selected = if count == 0 {
        0
    } else {
        view.selected.min(count - 1)
    };
    let caption = match view.status {
        Some(status) => format!("{count} Networks / {} {status}", view.spinner),
        None => format!("{count} Networks"),
    };
    let header_color = view.theme.secondary;
    let known_count = view.networks.iter().filter(|network| network.known).count();
    let new_count = count - known_count;

    let mut rows: Vec<AnyElement<'static>> = Vec::new();
    if known_count > 0 {
        rows.push(section_header("Known Networks", view.theme.secondary));
        rows.push(table_header(
            &[
                ("Name", KNOWN_NAME_PCT),
                ("Security", KNOWN_SECURITY_PCT),
                ("Hidden", KNOWN_HIDDEN_PCT),
                ("Auto Connect", KNOWN_AUTO_CONNECT_PCT),
                ("Signal", KNOWN_SIGNAL_PCT),
            ],
            header_color,
        ));
        rows.push(table_separator());
        for (index, network) in view
            .networks
            .iter()
            .enumerate()
            .filter(|(_, network)| network.known)
        {
            rows.push(known_row(
                view.theme,
                view.table_width,
                network,
                index == selected,
            ));
        }
    }

    if new_count > 0 {
        if known_count > 0 {
            rows.push(spacer());
        }
        rows.push(section_header("New Networks", view.theme.secondary));
        rows.push(table_header(
            &[
                ("Name", NEW_NAME_PCT),
                ("Security", NEW_SECURITY_PCT),
                ("Signal", NEW_SIGNAL_PCT),
            ],
            header_color,
        ));
        rows.push(table_separator());
        for (index, network) in view
            .networks
            .iter()
            .enumerate()
            .filter(|(_, network)| !network.known)
        {
            rows.push(new_row(
                view.theme,
                view.table_width,
                network,
                index == selected,
            ));
        }
    }

    let empty: Option<AnyElement<'static>> = (count == 0).then(|| {
        element! {
            Text(
                content: "No networks found. Press s to scan.",
                color: Color::Grey,
            )
        }
        .into_any()
    });
    element! {
        View(
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            padding: 1,
            border_style: BorderStyle::Round,
            border_color: if view.focused { view.theme.primary } else { Color::DarkGrey },
        ) {
            View(
                top: -1,
                left: 1,
                background_color: view.theme.background,
                position: Position::Absolute,
                padding_left: 1,
                padding_right: 1,
            ) {
                Text(
                    content: "NETWORKS",
                    color: if view.focused { view.theme.primary } else { Color::Grey },
                    weight: if view.focused { Weight::Bold } else { Weight::Normal },
                )
            }
            View(
                top: -1,
                right: 1,
                background_color: view.theme.background,
                position: Position::Absolute,
                padding_left: 1,
                padding_right: 1,
            ) {
                Text(
                    content: caption,
                    color: if view.focused { view.theme.primary } else { Color::Grey },
                    weight: if view.focused { Weight::Bold } else { Weight::Normal },
                )
            }

            View(flex_grow: 1.0, width: 100pct) {
                ScrollView(handle: view.handle, keyboard_scroll: false, scrollbar: false) {
                    View(flex_direction: FlexDirection::Column, width: 100pct) {
                        #(empty)
                        #(rows)
                    }
                }
            }

        }
    }
    .into_any()
}
