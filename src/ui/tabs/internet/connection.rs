use crate::network::{WifiState, frequency_label};
use crate::ui::common::Theme;
use iocraft::prelude::*;

pub fn connection_panel(
    theme: Theme,
    focused: bool,
    wifi: Option<&WifiState>,
    status: Option<&str>,
    spinner: &str,
) -> AnyElement<'static> {
    let active = wifi.and_then(|wifi| wifi.active.as_ref());
    let interface = wifi.map(|wifi| wifi.interface.as_str()).unwrap_or("--");
    let ip = active
        .and_then(|active| active.ip.as_deref())
        .unwrap_or("--");
    let security = active.map(|active| active.security.label()).unwrap_or("--");
    let frequency = active
        .map(|active| frequency_label(active.frequency))
        .unwrap_or_else(|| "--".to_string());

    let first_line = match status {
        Some(status) => element! {
            Text(
                align: TextAlign::Center,
                content: format!("{spinner}  {status}"),
                color: theme.secondary,
                weight: Weight::Bold,
            )
        }
        .into_any(),
        None => match active {
            Some(active) => element! {
                Text(
                    align: TextAlign::Center,
                    content: format!("󰤥  {} 󰧟 {}%", active.ssid, active.strength),
                    color: theme.primary,
                    weight: Weight::Bold,
                )
            }
            .into_any(),
            None => element! {
                Text(
                    align: TextAlign::Center,
                    content: "󰤮  Not connected",
                    color: Color::Grey,
                    weight: Weight::Bold,
                )
            }
            .into_any(),
        },
    };

    element! {
        View(
            flex_direction: FlexDirection::Column,
            border_style: BorderStyle::Round,
            border_color: if focused { theme.primary } else { Color::DarkGrey },
            padding: 1,
        ) {
            View(
                top: -1,
                left: 1,
                background_color: theme.background,
                position: Position::Absolute,
                padding_left: 1,
                padding_right: 1,
            ) {
                Text(
                    content: "CONNECTION",
                    color: if focused { theme.primary } else { Color::Grey },
                    weight: if focused { Weight::Bold } else { Weight::Normal },
                )
            }

            View(
                justify_content: JustifyContent::SpaceEvenly,
                flex_wrap: FlexWrap::Wrap,
            ) {
                #(Some(first_line))
                Text(
                    align: TextAlign::Center,
                    content: format!("󰩟 {interface} 󰧟 {ip}"),
                    color: Color::Cyan,
                )
                View(
                    width: 100pct,
                    border_style: BorderStyle::Round,
                    border_color: Color::DarkGrey,
                    border_edges: Edges::Bottom,
                )
                Text(
                    align: TextAlign::Center,
                    content: format!(" Security 󰧟 {security}"),
                )
                View(
                    border_style: BorderStyle::Round,
                    border_color: Color::DarkGrey,
                    border_edges: Edges::Right,
                )
                Text(
                    align: TextAlign::Center,
                    content: format!("󱑽 Frequency 󰧟 {frequency}"),
                )
                View(
                    border_style: BorderStyle::Round,
                    border_color: Color::DarkGrey,
                    border_edges: Edges::Right,
                )
                Text(
                    align: TextAlign::Center,
                    content: " Mode 󰧟 Station",
                )
            }
        }
    }
    .into_any()
}
