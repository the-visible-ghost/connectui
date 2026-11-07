use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;

use zbus::{OwnedObjectPath, OwnedVariant, Variant};

use super::types::Security;

pub type SettingMap = HashMap<String, OwnedVariant>;
pub type Settings = HashMap<String, SettingMap>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceKind {
    Wifi,
    Ethernet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceInfo {
    pub path: OwnedObjectPath,
    pub interface: String,
    pub kind: DeviceKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionType {
    Wifi,
    Ethernet,
}

impl ConnectionType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Wifi => "Wi-Fi",
            Self::Ethernet => "Ethernet",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WifiMode {
    Infrastructure,
    Adhoc,
    Ap,
    Custom(String),
}

impl WifiMode {
    pub fn label(&self) -> String {
        match self {
            Self::Infrastructure => "Infrastructure".to_string(),
            Self::Adhoc => "Ad-Hoc".to_string(),
            Self::Ap => "Access Point".to_string(),
            Self::Custom(value) => value.clone(),
        }
    }

    pub fn value(&self) -> &str {
        match self {
            Self::Infrastructure => "infrastructure",
            Self::Adhoc => "adhoc",
            Self::Ap => "ap",
            Self::Custom(value) => value,
        }
    }

    fn from_value(value: &str) -> Self {
        match value {
            "infrastructure" => Self::Infrastructure,
            "adhoc" => Self::Adhoc,
            "ap" => Self::Ap,
            other => Self::Custom(other.to_string()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IpMethod {
    Disabled,
    #[default]
    Automatic,
    LinkLocal,
    Manual,
    Shared,
}

impl IpMethod {
    pub const OPTIONS: [Self; 5] = [
        Self::Disabled,
        Self::Automatic,
        Self::LinkLocal,
        Self::Manual,
        Self::Shared,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::Automatic => "Automatic",
            Self::LinkLocal => "Link-Local",
            Self::Manual => "Manual",
            Self::Shared => "Shared",
        }
    }

    fn setting_value(self, ipv6: bool) -> &'static str {
        match self {
            Self::Disabled if ipv6 => "ignore",
            Self::Disabled => "disabled",
            Self::Automatic => "auto",
            Self::LinkLocal => "link-local",
            Self::Manual => "manual",
            Self::Shared => "shared",
        }
    }

    fn from_setting(value: &str, ipv6: bool) -> Self {
        match value {
            "disabled" if !ipv6 => Self::Disabled,
            "ignore" if ipv6 => Self::Disabled,
            "auto" => Self::Automatic,
            "link-local" => Self::LinkLocal,
            "manual" => Self::Manual,
            "shared" => Self::Shared,
            _ => Self::Automatic,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IpAddress {
    pub address: String,
    pub prefix: u32,
}

impl IpAddress {
    pub fn display(&self) -> String {
        format!("{}/{}", self.address, self.prefix)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IpConfig {
    pub method: IpMethod,
    pub addresses: Vec<IpAddress>,
    pub gateway: String,
    pub dns: Vec<String>,
    pub search: Vec<String>,
}

impl Default for IpConfig {
    fn default() -> Self {
        Self {
            method: IpMethod::Automatic,
            addresses: Vec::new(),
            gateway: String::new(),
            dns: Vec::new(),
            search: Vec::new(),
        }
    }
}

impl IpConfig {
    pub fn addresses_text(&self) -> String {
        self.addresses
            .iter()
            .map(IpAddress::display)
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn dns_text(&self) -> String {
        self.dns.join(", ")
    }

    pub fn search_text(&self) -> String {
        self.search.join(", ")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WifiConfig {
    pub ssid: String,
    pub mode: WifiMode,
    pub security: Security,
    pub password: Option<String>,
    pub password_set: bool,
    pub bssid: String,
    pub cloned_mac_address: String,
    pub mtu: Option<u32>,
    pub hidden: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EthernetConfig {
    pub mac_address: String,
    pub cloned_mac_address: String,
    pub mtu: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileDraft {
    pub path: Option<OwnedObjectPath>,
    pub connection_type: ConnectionType,
    pub id: String,
    pub uuid: String,
    pub interface_name: String,
    pub autoconnect: bool,
    pub all_users: bool,
    pub permissions: Vec<String>,
    pub wifi: Option<WifiConfig>,
    pub ethernet: Option<EthernetConfig>,
    pub ipv4: IpConfig,
    pub ipv6: IpConfig,
}

impl ProfileDraft {
    pub fn new_wifi(ssid: &str, security: Security, interface: &str) -> Self {
        Self {
            path: None,
            connection_type: ConnectionType::Wifi,
            id: ssid.to_string(),
            uuid: new_uuid(),
            interface_name: interface.to_string(),
            autoconnect: true,
            all_users: true,
            permissions: Vec::new(),
            wifi: Some(WifiConfig {
                ssid: ssid.to_string(),
                mode: WifiMode::Infrastructure,
                security,
                password: None,
                password_set: false,
                bssid: String::new(),
                cloned_mac_address: String::new(),
                mtu: None,
                hidden: false,
            }),
            ethernet: None,
            ipv4: IpConfig::default(),
            ipv6: IpConfig::default(),
        }
    }
}

pub fn draft_from_settings(
    path: OwnedObjectPath,
    settings: &Settings,
) -> Result<ProfileDraft, String> {
    let connection = settings
        .get("connection")
        .ok_or_else(|| "Profile has no connection settings".to_string())?;
    let connection_type = match string_prop(connection, "type").as_deref() {
        Some("802-11-wireless") => ConnectionType::Wifi,
        Some("802-3-ethernet") => ConnectionType::Ethernet,
        Some(other) => return Err(format!("Editing {other} profiles is not supported")),
        None => return Err("Profile type is missing".to_string()),
    };
    let permissions = vec_prop::<String>(connection, "permissions").unwrap_or_default();

    let mut draft = ProfileDraft {
        path: Some(path),
        connection_type,
        id: string_prop(connection, "id").unwrap_or_default(),
        uuid: string_prop(connection, "uuid").unwrap_or_default(),
        interface_name: string_prop(connection, "interface-name").unwrap_or_default(),
        autoconnect: bool_prop(connection, "autoconnect").unwrap_or(true),
        all_users: permissions.is_empty(),
        permissions,
        wifi: None,
        ethernet: None,
        ipv4: ip_config(settings.get("ipv4"), false),
        ipv6: ip_config(settings.get("ipv6"), true),
    };

    match connection_type {
        ConnectionType::Wifi => {
            let wireless = settings
                .get("802-11-wireless")
                .ok_or_else(|| "Profile has no Wi-Fi settings".to_string())?;
            let security = settings.get("802-11-wireless-security");
            let security_type = parse_security(security);
            let password = security.and_then(|settings| {
                string_prop(settings, "psk").or_else(|| string_prop(settings, "wep-key0"))
            });
            let password = password.filter(|value| !value.is_empty());
            draft.wifi = Some(WifiConfig {
                ssid: bytes_string(wireless, "ssid").unwrap_or_default(),
                mode: WifiMode::from_value(
                    string_prop(wireless, "mode")
                        .as_deref()
                        .unwrap_or("infrastructure"),
                ),
                security: security_type,
                password_set: password.is_some() || needs_password(security_type),
                password,
                bssid: string_prop(wireless, "bssid")
                    .or_else(|| mac_prop(wireless, "bssid"))
                    .unwrap_or_default(),
                cloned_mac_address: string_prop(wireless, "cloned-mac-address")
                    .or_else(|| mac_prop(wireless, "cloned-mac-address"))
                    .unwrap_or_default(),
                mtu: u32_prop(wireless, "mtu").filter(|value| *value > 0),
                hidden: bool_prop(wireless, "hidden").unwrap_or(false),
            });
        }
        ConnectionType::Ethernet => {
            let ethernet = settings
                .get("802-3-ethernet")
                .ok_or_else(|| "Profile has no Ethernet settings".to_string())?;
            draft.ethernet = Some(EthernetConfig {
                mac_address: mac_prop(ethernet, "mac-address")
                    .or_else(|| string_prop(ethernet, "mac-address"))
                    .unwrap_or_default(),
                cloned_mac_address: string_prop(ethernet, "cloned-mac-address")
                    .or_else(|| mac_prop(ethernet, "cloned-mac-address"))
                    .unwrap_or_default(),
                mtu: u32_prop(ethernet, "mtu").filter(|value| *value > 0),
            });
        }
    }

    Ok(draft)
}

pub fn settings_from_draft(
    draft: &ProfileDraft,
    existing: Option<Settings>,
) -> Result<Settings, String> {
    let mut settings = existing.unwrap_or_default();
    let id = draft.id.trim();
    if id.is_empty() {
        return Err("Profile name cannot be empty".to_string());
    }
    let uuid = if draft.uuid.trim().is_empty() {
        new_uuid()
    } else {
        draft.uuid.clone()
    };

    {
        let connection = settings.entry("connection".to_string()).or_default();
        set_string(connection, "id", id);
        set_string(
            connection,
            "type",
            connection_type_value(draft.connection_type),
        );
        set_string(connection, "uuid", &uuid);
        set_optional_string(connection, "interface-name", &draft.interface_name);
        set_bool(connection, "autoconnect", draft.autoconnect);
        let permissions = if draft.all_users {
            Vec::new()
        } else if draft.permissions.is_empty() {
            vec![format!("user:{}", current_user())]
        } else {
            draft.permissions.clone()
        };
        if permissions.is_empty() {
            connection.remove("permissions");
        } else {
            connection.insert("permissions".to_string(), owned(permissions));
        }
    }

    match draft.connection_type {
        ConnectionType::Wifi => {
            let wifi = draft
                .wifi
                .as_ref()
                .ok_or_else(|| "Wi-Fi settings are missing".to_string())?;
            if wifi.ssid.is_empty() {
                return Err("SSID cannot be empty".to_string());
            }
            let wireless = settings.entry("802-11-wireless".to_string()).or_default();
            wireless.insert("ssid".to_string(), owned(wifi.ssid.as_bytes().to_vec()));
            set_string(wireless, "mode", wifi.mode.value());
            set_bool(wireless, "hidden", wifi.hidden);
            if wifi.bssid.trim().is_empty() {
                wireless.remove("bssid");
            } else {
                let bytes = parse_mac(&wifi.bssid)
                    .ok_or_else(|| "BSSID must contain six octets".to_string())?;
                wireless.insert("bssid".to_string(), owned(bytes));
            }
            set_optional_string(wireless, "cloned-mac-address", &wifi.cloned_mac_address);
            set_optional_u32(wireless, "mtu", wifi.mtu);
            apply_security(&mut settings, wifi, draft.path.is_none())?;
        }
        ConnectionType::Ethernet => {
            let ethernet = draft
                .ethernet
                .as_ref()
                .ok_or_else(|| "Ethernet settings are missing".to_string())?;
            let section = settings.entry("802-3-ethernet".to_string()).or_default();
            if ethernet.mac_address.trim().is_empty() {
                section.remove("mac-address");
            } else {
                let bytes = parse_mac(&ethernet.mac_address)
                    .ok_or_else(|| "MAC address must contain six octets".to_string())?;
                section.insert("mac-address".to_string(), owned(bytes));
            }
            set_optional_string(section, "cloned-mac-address", &ethernet.cloned_mac_address);
            set_optional_u32(section, "mtu", ethernet.mtu);
        }
    }

    apply_ip(&mut settings, "ipv4", &draft.ipv4, false)?;
    apply_ip(&mut settings, "ipv6", &draft.ipv6, true)?;

    Ok(settings)
}

fn apply_security(settings: &mut Settings, wifi: &WifiConfig, is_new: bool) -> Result<(), String> {
    const SECTION: &str = "802-11-wireless-security";
    if wifi.security == Security::Open {
        settings.remove(SECTION);
        return Ok(());
    }

    let desired = match wifi.security {
        Security::Open => unreachable!(),
        Security::Wep => "none",
        Security::Wpa | Security::Wpa2 => "wpa-psk",
        Security::Wpa3 => "sae",
        Security::Owe => "owe",
        Security::Enterprise => "enterprise",
    };

    if wifi.security == Security::Enterprise {
        let section = settings
            .get(SECTION)
            .ok_or_else(|| "Enterprise security settings are missing".to_string())?;
        let current = string_prop(section, "key-mgmt").unwrap_or_default();
        if !matches!(
            current.as_str(),
            "wpa-eap" | "ieee8021x" | "wpa-eap-suite-b-192"
        ) {
            return Err("A new enterprise profile cannot be created here".to_string());
        }
        return Ok(());
    }

    let section = settings.entry(SECTION.to_string()).or_default();
    let current = string_prop(section, "key-mgmt");
    let same_security = current.as_deref() == Some(desired);
    if !same_security {
        section.clear();
    }
    set_string(section, "key-mgmt", desired);

    match wifi.security {
        Security::Wep => {
            section.remove("psk");
            if let Some(password) = wifi.password.as_deref().filter(|value| !value.is_empty()) {
                set_string(section, "wep-key0", password);
            } else if is_new || (!same_security && !section.contains_key("wep-key0")) {
                return Err("A WEP key is required".to_string());
            }
        }
        Security::Wpa | Security::Wpa2 | Security::Wpa3 => {
            section.remove("wep-key0");
            if let Some(password) = wifi.password.as_deref().filter(|value| !value.is_empty()) {
                set_string(section, "psk", password);
            } else if is_new || (!same_security && !section.contains_key("psk")) {
                return Err("A Wi-Fi password is required".to_string());
            }
        }
        Security::Owe => {
            section.remove("psk");
            section.remove("wep-key0");
        }
        Security::Open | Security::Enterprise => {}
    }

    Ok(())
}

fn apply_ip(
    settings: &mut Settings,
    name: &str,
    config: &IpConfig,
    ipv6: bool,
) -> Result<(), String> {
    let section = settings.entry(name.to_string()).or_default();
    set_string(section, "method", config.method.setting_value(ipv6));

    if config.method == IpMethod::Manual {
        if config.addresses.is_empty() {
            return Err(format!(
                "{} manual mode requires at least one address",
                if ipv6 { "IPv6" } else { "IPv4" }
            ));
        }
        let mut entries = Vec::with_capacity(config.addresses.len());
        for address in &config.addresses {
            let ip = IpAddr::from_str(&address.address)
                .map_err(|_| format!("Invalid address: {}", address.address))?;
            match (ipv6, ip) {
                (false, IpAddr::V4(_)) | (true, IpAddr::V6(_)) => {}
                _ => {
                    return Err(format!(
                        "{} is not an {} address",
                        address.address,
                        if ipv6 { "IPv6" } else { "IPv4" }
                    ));
                }
            }
            let max = if ipv6 { 128 } else { 32 };
            if address.prefix > max {
                return Err(format!("Invalid prefix: {}", address.prefix));
            }
            let mut entry = SettingMap::new();
            set_string(&mut entry, "address", &address.address);
            entry.insert("prefix".to_string(), owned(address.prefix));
            entries.push(entry);
        }
        section.insert("address-data".to_string(), owned(entries));
        section.remove("addresses");
        if !config.gateway.trim().is_empty() {
            let gateway = IpAddr::from_str(config.gateway.trim())
                .map_err(|_| format!("Invalid gateway: {}", config.gateway))?;
            match (ipv6, gateway) {
                (false, IpAddr::V4(_)) | (true, IpAddr::V6(_)) => {}
                _ => {
                    return Err(format!(
                        "{} is not an {} gateway",
                        config.gateway,
                        if ipv6 { "IPv6" } else { "IPv4" }
                    ));
                }
            }
        }
        set_optional_string(section, "gateway", &config.gateway);
    } else {
        section.remove("address-data");
        section.remove("addresses");
        section.remove("gateway");
    }

    if config.dns.is_empty() {
        section.remove("dns-data");
        section.remove("dns");
    } else {
        for dns in &config.dns {
            let ip = IpAddr::from_str(dns).map_err(|_| format!("Invalid DNS server: {dns}"))?;
            match (ipv6, ip) {
                (false, IpAddr::V4(_)) | (true, IpAddr::V6(_)) => {}
                _ => {
                    return Err(format!(
                        "{dns} is not an {} DNS server",
                        if ipv6 { "IPv6" } else { "IPv4" }
                    ));
                }
            }
        }
        section.insert("dns-data".to_string(), owned(config.dns.clone()));
        section.remove("dns");
    }

    if config.search.is_empty() {
        section.remove("dns-search");
    } else {
        section.insert("dns-search".to_string(), owned(config.search.clone()));
    }

    Ok(())
}

fn ip_config(section: Option<&SettingMap>, ipv6: bool) -> IpConfig {
    let Some(section) = section else {
        return IpConfig::default();
    };
    let method = string_prop(section, "method")
        .map(|value| IpMethod::from_setting(&value, ipv6))
        .unwrap_or_default();
    let addresses = address_data(section).unwrap_or_else(|| legacy_addresses(section, ipv6));
    let dns = dns_data(section).unwrap_or_else(|| legacy_dns(section, ipv6));
    IpConfig {
        method,
        addresses,
        gateway: string_prop(section, "gateway").unwrap_or_default(),
        dns,
        search: vec_prop(section, "dns-search").unwrap_or_default(),
    }
}

fn address_data(section: &SettingMap) -> Option<Vec<IpAddress>> {
    let entries = vec_prop::<SettingMap>(section, "address-data")?;
    Some(
        entries
            .into_iter()
            .filter_map(|entry| {
                let address = string_prop(&entry, "address")?;
                let prefix = u32_prop(&entry, "prefix").unwrap_or(0);
                Some(IpAddress { address, prefix })
            })
            .collect(),
    )
}

fn legacy_addresses(section: &SettingMap, ipv6: bool) -> Vec<IpAddress> {
    if ipv6 {
        vec_prop::<(Vec<u8>, u32, Vec<u8>)>(section, "addresses")
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(bytes, prefix, _)| {
                let address =
                    IpAddr::V6(std::net::Ipv6Addr::from(<[u8; 16]>::try_from(bytes).ok()?));
                Some(IpAddress {
                    address: address.to_string(),
                    prefix,
                })
            })
            .collect()
    } else {
        vec_prop::<Vec<u32>>(section, "addresses")
            .unwrap_or_default()
            .into_iter()
            .filter_map(|address| {
                let value = *address.first()?;
                Some(IpAddress {
                    address: std::net::Ipv4Addr::from(value).to_string(),
                    prefix: address.get(1).copied().unwrap_or(0),
                })
            })
            .collect()
    }
}

fn dns_data(section: &SettingMap) -> Option<Vec<String>> {
    vec_prop::<String>(section, "dns-data")
}

fn legacy_dns(section: &SettingMap, ipv6: bool) -> Vec<String> {
    if ipv6 {
        vec_prop::<Vec<u8>>(section, "dns")
            .unwrap_or_default()
            .into_iter()
            .filter_map(|bytes| {
                Some(std::net::Ipv6Addr::from(<[u8; 16]>::try_from(bytes).ok()?).to_string())
            })
            .collect()
    } else {
        vec_prop::<u32>(section, "dns")
            .unwrap_or_default()
            .into_iter()
            .map(|value| std::net::Ipv4Addr::from(value).to_string())
            .collect()
    }
}

fn needs_password(security: Security) -> bool {
    matches!(
        security,
        Security::Wep | Security::Wpa | Security::Wpa2 | Security::Wpa3
    )
}

fn parse_security(section: Option<&SettingMap>) -> Security {
    let Some(section) = section else {
        return Security::Open;
    };
    match string_prop(section, "key-mgmt").as_deref() {
        Some("none") => Security::Wep,
        Some("wpa-psk") => Security::Wpa2,
        Some("sae") => Security::Wpa3,
        Some("owe") => Security::Owe,
        Some("wpa-eap" | "ieee8021x" | "wpa-eap-suite-b-192") => Security::Enterprise,
        _ => Security::Open,
    }
}

pub fn parse_addresses(value: &str, ipv6: bool) -> Result<Vec<IpAddress>, String> {
    split_list(value)
        .into_iter()
        .map(|item| {
            let (address, prefix) = item
                .split_once('/')
                .ok_or_else(|| format!("Use address/prefix for {item}"))?;
            let ip =
                IpAddr::from_str(address).map_err(|_| format!("Invalid address: {address}"))?;
            match (ipv6, ip) {
                (false, IpAddr::V4(_)) | (true, IpAddr::V6(_)) => {}
                _ => {
                    return Err(format!(
                        "{address} is not an {} address",
                        if ipv6 { "IPv6" } else { "IPv4" }
                    ));
                }
            }
            let prefix = prefix
                .parse::<u32>()
                .map_err(|_| format!("Invalid prefix: {prefix}"))?;
            let max = if ipv6 { 128 } else { 32 };
            if prefix > max {
                return Err(format!("Invalid prefix: {prefix}"));
            }
            Ok(IpAddress {
                address: address.to_string(),
                prefix,
            })
        })
        .collect()
}

pub fn parse_ip_list(value: &str, ipv6: bool) -> Result<Vec<String>, String> {
    split_list(value)
        .into_iter()
        .map(|item| {
            let ip = IpAddr::from_str(&item).map_err(|_| format!("Invalid address: {item}"))?;
            match (ipv6, ip) {
                (false, IpAddr::V4(_)) | (true, IpAddr::V6(_)) => Ok(item),
                _ => Err(format!(
                    "{item} is not an {} address",
                    if ipv6 { "IPv6" } else { "IPv4" }
                )),
            }
        })
        .collect()
}

pub fn parse_search_list(value: &str) -> Vec<String> {
    split_list(value)
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split([' ', ',', '\t'])
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn connection_type_value(value: ConnectionType) -> &'static str {
    match value {
        ConnectionType::Wifi => "802-11-wireless",
        ConnectionType::Ethernet => "802-3-ethernet",
    }
}

fn current_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "root".to_string())
}

fn new_uuid() -> String {
    std::fs::read_to_string("/proc/sys/kernel/random/uuid")
        .map(|value| value.trim().to_string())
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            format!(
                "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
                now.as_secs() as u32,
                (now.as_secs() >> 32) as u16,
                now.subsec_micros() as u16,
                (now.subsec_nanos() >> 16) as u16,
                now.as_nanos() as u64 & 0xffff_ffffffff
            )
        })
}

fn parse_mac(value: &str) -> Option<Vec<u8>> {
    let parts: Vec<&str> = value.split([':', '-']).collect();
    if parts.len() != 6 {
        return None;
    }
    parts
        .iter()
        .map(|part| u8::from_str_radix(part, 16).ok())
        .collect()
}

fn owned<T>(value: T) -> OwnedVariant
where
    T: Into<Variant<'static>>,
{
    OwnedVariant::try_from(value.into()).expect("variant is owned")
}

fn set_string(map: &mut SettingMap, key: &str, value: &str) {
    map.insert(key.to_string(), owned(value.to_string()));
}

fn set_optional_string(map: &mut SettingMap, key: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        map.remove(key);
    } else {
        set_string(map, key, value);
    }
}

fn set_bool(map: &mut SettingMap, key: &str, value: bool) {
    map.insert(key.to_string(), owned(value));
}

fn set_optional_u32(map: &mut SettingMap, key: &str, value: Option<u32>) {
    match value {
        Some(value) => {
            map.insert(key.to_string(), owned(value));
        }
        None => {
            map.remove(key);
        }
    }
}

fn string_prop(map: &SettingMap, key: &str) -> Option<String> {
    map.get(key)?.try_clone().ok()?.try_into().ok()
}

fn bytes_string(map: &SettingMap, key: &str) -> Option<String> {
    let bytes = vec_prop::<u8>(map, key)?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn mac_prop(map: &SettingMap, key: &str) -> Option<String> {
    let bytes = vec_prop::<u8>(map, key)?;
    if bytes.len() != 6 {
        return None;
    }
    Some(
        bytes
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(":"),
    )
}

fn bool_prop(map: &SettingMap, key: &str) -> Option<bool> {
    map.get(key)?.try_clone().ok()?.try_into().ok()
}

fn u32_prop(map: &SettingMap, key: &str) -> Option<u32> {
    map.get(key)?.try_clone().ok()?.try_into().ok()
}

fn vec_prop<T>(map: &SettingMap, key: &str) -> Option<Vec<T>>
where
    T: TryFrom<Variant<'static>>,
    Vec<T>: TryFrom<OwnedVariant>,
{
    map.get(key)?.try_clone().ok()?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_path() -> OwnedObjectPath {
        OwnedObjectPath::try_from("/org/freedesktop/NetworkManager/Settings/1").unwrap()
    }

    fn wifi_draft() -> ProfileDraft {
        let mut draft = ProfileDraft::new_wifi("TestNet", Security::Wpa2, "wlan0");
        let wifi = draft.wifi.as_mut().unwrap();
        wifi.password = Some("correct horse".to_string());
        wifi.password_set = true;
        wifi.bssid = "AA:BB:CC:DD:EE:FF".to_string();
        wifi.mtu = Some(1400);
        draft.ipv4 = IpConfig {
            method: IpMethod::Manual,
            addresses: vec![IpAddress {
                address: "192.0.2.10".to_string(),
                prefix: 24,
            }],
            gateway: "192.0.2.1".to_string(),
            dns: vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()],
            search: vec!["lan".to_string()],
        };
        draft.ipv6 = IpConfig {
            method: IpMethod::Disabled,
            addresses: Vec::new(),
            gateway: String::new(),
            dns: Vec::new(),
            search: Vec::new(),
        };
        draft
    }

    #[test]
    fn wifi_round_trip_preserves_settings() {
        let draft = wifi_draft();
        let mut settings = settings_from_draft(&draft, None).unwrap();
        settings
            .get_mut("connection")
            .unwrap()
            .insert("timestamp".to_string(), owned(123u32));
        settings
            .get_mut("ipv4")
            .unwrap()
            .insert("route-metric".to_string(), owned(100u32));
        settings
            .get_mut("802-11-wireless")
            .unwrap()
            .insert("powersave".to_string(), owned(2u32));

        let mut loaded = draft_from_settings(profile_path(), &settings).unwrap();
        assert_eq!(loaded.id, "TestNet");
        assert_eq!(loaded.connection_type, ConnectionType::Wifi);
        assert_eq!(loaded.ipv4.addresses_text(), "192.0.2.10/24");
        assert_eq!(loaded.ipv4.gateway, "192.0.2.1");
        assert_eq!(loaded.ipv4.dns_text(), "1.1.1.1, 8.8.8.8");
        assert_eq!(loaded.ipv6.method, IpMethod::Disabled);
        let wifi = loaded.wifi.as_ref().unwrap();
        assert_eq!(wifi.security, Security::Wpa2);
        assert_eq!(wifi.password.as_deref(), Some("correct horse"));
        assert!(wifi.password_set);
        assert_eq!(wifi.bssid, "AA:BB:CC:DD:EE:FF");
        assert_eq!(wifi.mtu, Some(1400));

        loaded.id = "Renamed".to_string();
        let saved = settings_from_draft(&loaded, Some(settings)).unwrap();
        let connection = saved.get("connection").unwrap();
        assert_eq!(string_prop(connection, "id").as_deref(), Some("Renamed"));
        assert_eq!(u32_prop(connection, "timestamp"), Some(123));
        let ipv4 = saved.get("ipv4").unwrap();
        assert_eq!(u32_prop(ipv4, "route-metric"), Some(100));
        assert_eq!(string_prop(ipv4, "method").as_deref(), Some("manual"));
        assert!(!ipv4.contains_key("addresses"));
        assert_eq!(
            address_data(ipv4),
            Some(vec![IpAddress {
                address: "192.0.2.10".to_string(),
                prefix: 24,
            }])
        );
        assert_eq!(
            dns_data(ipv4),
            Some(vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()])
        );
        let ipv6 = saved.get("ipv6").unwrap();
        assert_eq!(string_prop(ipv6, "method").as_deref(), Some("ignore"));
        let wireless = saved.get("802-11-wireless").unwrap();
        assert_eq!(u32_prop(wireless, "powersave"), Some(2));
        assert_eq!(
            vec_prop::<u8>(wireless, "bssid"),
            Some(vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF])
        );
        let security = saved.get("802-11-wireless-security").unwrap();
        assert_eq!(
            string_prop(security, "key-mgmt").as_deref(),
            Some("wpa-psk")
        );
        assert_eq!(
            string_prop(security, "psk").as_deref(),
            Some("correct horse")
        );
    }

    #[test]
    fn open_security_removes_section() {
        let mut draft = wifi_draft();
        let existing = settings_from_draft(&draft, None).unwrap();
        draft.wifi.as_mut().unwrap().security = Security::Open;
        let saved = settings_from_draft(&draft, Some(existing)).unwrap();
        assert!(!saved.contains_key("802-11-wireless-security"));
    }

    #[test]
    fn enterprise_security_is_preserved_but_not_created() {
        let mut draft = wifi_draft();
        draft.path = Some(profile_path());
        draft.wifi.as_mut().unwrap().security = Security::Enterprise;
        let mut existing = Settings::new();
        let mut security = SettingMap::new();
        set_string(&mut security, "key-mgmt", "wpa-eap");
        set_string(&mut security, "identity", "user");
        existing.insert("802-11-wireless-security".to_string(), security);
        let saved = settings_from_draft(&draft, Some(existing)).unwrap();
        let security = saved.get("802-11-wireless-security").unwrap();
        assert_eq!(
            string_prop(security, "key-mgmt").as_deref(),
            Some("wpa-eap")
        );
        assert_eq!(string_prop(security, "identity").as_deref(), Some("user"));

        let mut fresh = wifi_draft();
        fresh.path = None;
        fresh.wifi.as_mut().unwrap().security = Security::Enterprise;
        assert!(settings_from_draft(&fresh, None).is_err());
    }

    #[test]
    fn ethernet_round_trip() {
        let mut draft = ProfileDraft::new_wifi("Wired", Security::Open, "eth0");
        draft.connection_type = ConnectionType::Ethernet;
        draft.wifi = None;
        draft.ethernet = Some(EthernetConfig {
            mac_address: "00:11:22:33:44:55".to_string(),
            cloned_mac_address: String::new(),
            mtu: Some(1500),
        });
        let settings = settings_from_draft(&draft, None).unwrap();
        let loaded = draft_from_settings(profile_path(), &settings).unwrap();
        assert_eq!(loaded.connection_type, ConnectionType::Ethernet);
        assert!(loaded.wifi.is_none());
        let ethernet = loaded.ethernet.unwrap();
        assert_eq!(ethernet.mac_address, "00:11:22:33:44:55");
        assert_eq!(ethernet.mtu, Some(1500));
    }

    #[test]
    fn invalid_bssid_is_rejected() {
        let mut draft = wifi_draft();
        draft.wifi.as_mut().unwrap().bssid = "not-a-mac".to_string();
        assert_eq!(
            settings_from_draft(&draft, None).unwrap_err(),
            "BSSID must contain six octets"
        );
    }
}
