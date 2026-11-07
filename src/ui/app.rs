use std::time::Duration;

use crate::ui::common::{Tab, Theme};
use crate::ui::notification::{Notification, notification_overlay};
use crate::ui::tabs::bluetooth::BluetoothContent;
use crate::ui::tabs::internet::InternetContent;
use crate::ui::tabs::vpn::VpnContent;
use iocraft::prelude::*;

#[component]
pub fn FullScreen(mut hooks: Hooks) -> impl Into<AnyElement<'static>> {
    let (width, height) = hooks.use_terminal_size();
    let mut system = hooks.use_context_mut::<SystemContext>();
    let mut tab = hooks.use_state(|| Tab::Internet);
    let mut should_exit = hooks.use_state(|| false);
    let mut modal_active = hooks.use_state(|| false);
    let mut footer_override = hooks.use_state(|| None::<String>);
    let mut notifications = hooks.use_state(Vec::<Notification>::new);

    hooks.use_future(async move {
        loop {
            smol::Timer::after(Duration::from_millis(200)).await;
            let mut items = (*notifications.read()).clone();
            let before = items.len();
            items.retain(|notification| !notification.is_expired());
            if items.len() != before {
                notifications.set(items);
            }
        }
    });

    hooks.use_terminal_events({
        move |event| match event {
            TerminalEvent::Key(KeyEvent { code, kind, .. }) if kind != KeyEventKind::Release => {
                if modal_active.get() {
                    return;
                }
                match code {
                    KeyCode::Char('q') => should_exit.set(true),
                    KeyCode::Char('i') => {
                        tab.set(Tab::Internet);
                        footer_override.set(None);
                    }
                    KeyCode::Char('b') => {
                        tab.set(Tab::Bluetooth);
                        footer_override.set(None);
                    }
                    KeyCode::Char('v') => {
                        tab.set(Tab::VPN);
                        footer_override.set(None);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    });

    if should_exit.get() {
        system.exit();
    }

    let notification_items = (*notifications.read()).clone();
    let footer = footer_override.read().clone().unwrap_or_else(|| {
        if width < 120 {
            "󱁐/󰌑 Connect | d Remove | s Scan | j/k Move | Tab Panel | q Quit".to_string()
        } else {
            "󱁐 or 󰌑 Dis/connect  |  d Remove  |  s scan  |  k or \u{eaa1}  Up  |  j or \u{ea9a}  Down  |  (Shift+)Tab Panel  |  q Quit".to_string()
        }
    });
    let theme = match tab.get() {
        Tab::Internet => Theme {
            primary: Color::Green,
            primary_dark: Color::DarkGreen,
            secondary: Color::Yellow,
            secondary_dark: Color::DarkYellow,
            background: Color::Reset,
        },
        Tab::Bluetooth => Theme {
            primary: Color::Blue,
            primary_dark: Color::DarkBlue,
            secondary: Color::Yellow,
            secondary_dark: Color::DarkYellow,
            background: Color::Reset,
        },
        Tab::VPN => Theme {
            primary: Color::Red,
            primary_dark: Color::DarkRed,
            secondary: Color::Yellow,
            secondary_dark: Color::DarkYellow,
            background: Color::Reset,
        },
    };

    element! {
        View(width, height, max_height: height,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            background_color: Color::Reset,
        ) {
            View(
                border_style: BorderStyle::Round,
                border_color: theme.primary_dark,
                flex_grow: 1.0, width: 99pct,
                flex_direction: FlexDirection::Column,
                padding: 1,
                padding_bottom: 0, margin_top: 1,
            ) {
                View(
                    gap: 2, left: -1, width: 100pct,
                    position: Position::Absolute,
                    justify_content: JustifyContent::Center,
                    top: -2,
                ) {
                    Button(handler: move |_| {
                        if modal_active.get() {
                            return;
                        }
                        tab.set(Tab::Internet);
                        modal_active.set(false);
                        footer_override.set(None);
                    }) {
                        View(
                            border_color: match tab.get() {
                                Tab::Internet => theme.primary_dark,
                                _ => Color::DarkGrey
                            },
                            border_style: BorderStyle::Round,
                            padding_left: 2, padding_right: 2,
                            justify_content: JustifyContent::Center,
                            background_color: Color::Reset,
                        ) {
                            Text(
                                content: "󰖩  Internet | i",
                                weight: Weight::Bold,
                                color: match tab.get() {
                                    Tab::Internet => theme.primary,
                                    _ => Color::Reset
                                }
                            )
                        }
                    }
                    Button(handler: move |_| {
                        if modal_active.get() {
                            return;
                        }
                        tab.set(Tab::Bluetooth);
                        modal_active.set(false);
                        footer_override.set(None);
                    }) {
                        View(
                            border_color: match tab.get() {
                                Tab::Bluetooth => theme.primary_dark,
                                _ => Color::DarkGrey
                            },
                            border_style: BorderStyle::Round,
                            padding_left: 2, padding_right: 2,
                            justify_content: JustifyContent::Center,
                            background_color: Color::Reset,
                        ) {
                            Text(
                                content: "󰂯 Bluetooth | b",
                                weight: Weight::Bold,
                                color: match tab.get() {
                                    Tab::Bluetooth => theme.primary,
                                    _ => Color::Reset
                                }
                            )
                        }
                    }
                    Button(handler: move |_| {
                        if modal_active.get() {
                            return;
                        }
                        tab.set(Tab::VPN);
                        modal_active.set(false);
                        footer_override.set(None);
                    }) {
                        View(
                            border_color: match tab.get() {
                                Tab::VPN => theme.primary_dark,
                                _ => Color::DarkGrey
                            },
                            border_style: BorderStyle::Round,
                            padding_left: 2, padding_right: 2,
                            justify_content: JustifyContent::Center,
                            background_color: Color::Reset,
                        ) {
                            Text(
                                content: "󰫛 VPN | v",
                                weight: Weight::Bold,
                                color: match tab.get() {
                                    Tab::VPN => theme.primary,
                                    _ => Color::Reset
                                }
                            )
                        }
                    }
                }

                #(match tab.get() {
                    Tab::Internet => element!(InternetContent(
                        theme: theme,
                        modal_active: modal_active,
                        notifications: notifications,
                        footer: footer_override,
                    )).into_any(),
                    Tab::Bluetooth => element!(BluetoothContent(theme: theme)).into_any(),
                    Tab::VPN => element!(VpnContent(theme: theme)).into_any(),
                })
            }

            View(max_width: 100pct, justify_content: JustifyContent::Center, padding_left: 5, padding_right: 5) {
                Text(
                    content: footer,
                    color: Color::Grey, wrap: TextWrap::Wrap, align: TextAlign::Center
                )
            }

            #(Some(notification_overlay(&notification_items)))
        }
    }
}
