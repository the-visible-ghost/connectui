use crate::ui::notification::Notification;
use iocraft::prelude::*;

#[derive(Clone, Copy, Default)]
pub enum Tab {
    #[default]
    Internet,
    Bluetooth,
    VPN,
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub primary: Color,
    pub primary_dark: Color,
    pub secondary: Color,
    pub secondary_dark: Color,
    pub background: Color,
}

impl std::default::Default for Theme {
    fn default() -> Self {
        Self {
            primary: Color::White,
            primary_dark: Color::Grey,
            secondary: Color::Grey,
            secondary_dark: Color::DarkGrey,
            background: Color::Reset,
        }
    }
}

#[derive(Default, iocraft::Props)]
pub struct Props {
    pub active_tab: Tab,
    pub theme: Theme,
    pub modal_active: Option<State<bool>>,
    pub notifications: Option<State<Vec<Notification>>>,
}
