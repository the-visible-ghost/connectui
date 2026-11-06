use zbus::OwnedObjectPath;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Security {
    #[default]
    Open,
    Wep,
    Wpa,
    Wpa2,
    Wpa3,
    Owe,
    Enterprise,
}

impl Security {
    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Wep => "WEP",
            Self::Wpa => "WPA",
            Self::Wpa2 => "WPA2",
            Self::Wpa3 => "WPA3",
            Self::Owe => "OWE",
            Self::Enterprise => "802.1X",
        }
    }
}

pub fn frequency_label(frequency: u32) -> String {
    match frequency {
        0 => "--".to_string(),
        1..=2499 => "2.4 GHz".to_string(),
        2500..=5924 => "5 GHz".to_string(),
        _ => "6 GHz".to_string(),
    }
}

#[derive(Clone, Debug)]
pub struct WifiNetwork {
    pub ap_path: OwnedObjectPath,
    pub ssid: String,
    pub strength: u8,
    pub frequency: u32,
    pub security: Security,
    pub active: bool,
    pub known: bool,
    pub hidden: bool,
    pub auto_connect: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct KnownProfile {
    pub hidden: bool,
    pub auto_connect: bool,
}

#[derive(Clone, Debug)]
pub struct ActiveWifi {
    pub active_path: OwnedObjectPath,
    pub ssid: String,
    pub strength: u8,
    pub frequency: u32,
    pub security: Security,
    pub ip: Option<String>,
}

#[derive(Clone, Debug)]
pub struct WifiState {
    pub device_path: OwnedObjectPath,
    pub interface: String,
    pub active: Option<ActiveWifi>,
    pub networks: Vec<WifiNetwork>,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub wifi: Option<WifiState>,
}
