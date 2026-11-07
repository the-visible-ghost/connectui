use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use futures_lite::{Stream, StreamExt};
use zbus::{Connection, OwnedObjectPath, OwnedVariant, Proxy, Variant};

use super::profile::{
    DeviceInfo, DeviceKind, ProfileDraft, draft_from_settings, settings_from_draft,
};
use super::types::{ActiveWifi, KnownProfile, Security, Snapshot, WifiNetwork, WifiState};

const NM_DEST: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const NM_IFACE: &str = "org.freedesktop.NetworkManager";
const SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
const SETTINGS_IFACE: &str = "org.freedesktop.NetworkManager.Settings";
const SETTINGS_CONNECTION_IFACE: &str = "org.freedesktop.NetworkManager.Settings.Connection";
const DEVICE_IFACE: &str = "org.freedesktop.NetworkManager.Device";
const WIRELESS_IFACE: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const ACCESS_POINT_IFACE: &str = "org.freedesktop.NetworkManager.AccessPoint";
const ACTIVE_CONNECTION_IFACE: &str = "org.freedesktop.NetworkManager.Connection.Active";
const IP4_CONFIG_IFACE: &str = "org.freedesktop.NetworkManager.IP4Config";
const OBJECT_MANAGER_PATH: &str = "/org/freedesktop";
const OBJECT_MANAGER_IFACE: &str = "org.freedesktop.DBus.ObjectManager";

type PropMap = HashMap<String, OwnedVariant>;
type ObjectMap = HashMap<OwnedObjectPath, HashMap<String, PropMap>>;

#[derive(Debug)]
pub enum NetworkError {
    DBus(zbus::Error),
    Message(String),
    WrongPassword,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DBus(error) => write!(f, "{error}"),
            Self::Message(message) => f.write_str(message),
            Self::WrongPassword => f.write_str("Wrong password"),
        }
    }
}

impl std::error::Error for NetworkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DBus(error) => Some(error),
            Self::Message(_) | Self::WrongPassword => None,
        }
    }
}

impl From<zbus::Error> for NetworkError {
    fn from(error: zbus::Error) -> Self {
        Self::DBus(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectOutcome {
    Started,
    NeedsPassword,
    WrongPassword,
}

pub struct NMClient {
    connection: Connection,
}

impl NMClient {
    pub async fn new() -> Result<Self, NetworkError> {
        let connection = Connection::system().await?;
        let proxy = Proxy::new(&connection, NM_DEST, NM_PATH, NM_IFACE).await?;
        let _: String = proxy.get_property("Version").await?;
        Ok(Self { connection })
    }

    pub async fn snapshot(&self) -> Result<Snapshot, NetworkError> {
        let proxy = Proxy::new(
            &self.connection,
            NM_DEST,
            OBJECT_MANAGER_PATH,
            OBJECT_MANAGER_IFACE,
        )
        .await?;
        let objects: ObjectMap = proxy.call("GetManagedObjects", &()).await?;
        Ok(build_snapshot(&objects))
    }

    pub async fn request_scan(&self, device: &OwnedObjectPath) -> Result<(), NetworkError> {
        let proxy = Proxy::new(&self.connection, NM_DEST, device.as_str(), WIRELESS_IFACE).await?;
        let options: HashMap<&str, Variant<'_>> = HashMap::new();
        let _: () = proxy.call("RequestScan", &(options,)).await?;
        Ok(())
    }

    pub async fn connect(
        &self,
        device: &OwnedObjectPath,
        access_point: &OwnedObjectPath,
        ssid: &str,
        security: Security,
        password: Option<&str>,
    ) -> Result<ConnectOutcome, NetworkError> {
        if let Some(saved) = self.find_saved_connection(ssid).await? {
            if let Some(password) = password.filter(|_| needs_password(security)) {
                self.update_saved_password(&saved, security, password)
                    .await?;
            }
            let network_manager = Proxy::new(&self.connection, NM_DEST, NM_PATH, NM_IFACE).await?;
            let active: OwnedObjectPath = network_manager
                .call(
                    "ActivateConnection",
                    &(saved, device.clone(), access_point.clone()),
                )
                .await?;
            return match self
                .wait_for_activation(device, &active, needs_password(security))
                .await
            {
                Ok(()) => Ok(ConnectOutcome::Started),
                Err(NetworkError::WrongPassword) => Ok(ConnectOutcome::WrongPassword),
                Err(error) => Err(error),
            };
        }

        if security == Security::Enterprise {
            return Err(NetworkError::Message(
                "Enterprise networks are not supported yet".to_string(),
            ));
        }

        if needs_password(security) && password.is_none() {
            return Ok(ConnectOutcome::NeedsPassword);
        }

        let settings = connection_settings(ssid, security, password)?;
        let network_manager = Proxy::new(&self.connection, NM_DEST, NM_PATH, NM_IFACE).await?;
        let (created, active): (OwnedObjectPath, OwnedObjectPath) = network_manager
            .call(
                "AddAndActivateConnection",
                &(settings, device.clone(), access_point.clone()),
            )
            .await?;
        match self
            .wait_for_activation(device, &active, needs_password(security))
            .await
        {
            Ok(()) => Ok(ConnectOutcome::Started),
            Err(NetworkError::WrongPassword) => {
                let _ = self.delete_connection(&created).await;
                Ok(ConnectOutcome::WrongPassword)
            }
            Err(error) => {
                let _ = self.delete_connection(&created).await;
                Err(error)
            }
        }
    }

    async fn wait_for_activation(
        &self,
        device: &OwnedObjectPath,
        active_connection: &OwnedObjectPath,
        password_related: bool,
    ) -> Result<(), NetworkError> {
        const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(30);
        const ACTIVATED: u32 = 2;
        const DEACTIVATED: u32 = 4;
        const SIGNAL_WAIT: Duration = Duration::from_millis(250);
        const FAILURE_GRACE: Duration = Duration::from_millis(150);

        let active_proxy = Proxy::new(
            &self.connection,
            NM_DEST,
            active_connection.as_str(),
            ACTIVE_CONNECTION_IFACE,
        )
        .await?;
        let device_proxy =
            Proxy::new(&self.connection, NM_DEST, device.as_str(), DEVICE_IFACE).await?;
        let mut signals = device_proxy.receive_signal("StateChanged").await?;
        let deadline = Instant::now() + ACTIVATION_TIMEOUT;

        loop {
            match active_proxy.get_property::<u32>("State").await {
                Ok(ACTIVATED) => return Ok(()),
                Ok(DEACTIVATED) => {
                    let reason = smol::future::or(next_state_reason(&mut signals), async {
                        smol::Timer::after(FAILURE_GRACE).await;
                        None
                    })
                    .await;
                    if reason.is_some_and(is_wrong_password_reason) {
                        return Err(NetworkError::WrongPassword);
                    }
                    let device_reason = device_state_reason(&device_proxy).await;
                    if is_wrong_password_reason(device_reason) {
                        return Err(NetworkError::WrongPassword);
                    }
                    let active_reason = active_proxy
                        .get_property::<u32>("Reason")
                        .await
                        .unwrap_or(0);
                    let reason = if device_reason != 0 {
                        device_reason
                    } else {
                        active_reason
                    };
                    if password_related && reason == 0 {
                        return Err(NetworkError::WrongPassword);
                    }
                    return Err(NetworkError::Message(activation_error(reason)));
                }
                Ok(_) => {}
                Err(_) => {
                    return Err(NetworkError::Message("Connection failed".to_string()));
                }
            }
            if Instant::now() >= deadline {
                return Err(NetworkError::Message("Connection timed out".to_string()));
            }

            let reason = smol::future::or(next_state_reason(&mut signals), async {
                smol::Timer::after(SIGNAL_WAIT).await;
                None
            })
            .await;
            if reason.is_some_and(is_wrong_password_reason) {
                return Err(NetworkError::WrongPassword);
            }
        }
    }

    async fn update_saved_password(
        &self,
        connection: &OwnedObjectPath,
        security: Security,
        password: &str,
    ) -> Result<(), NetworkError> {
        let proxy = Proxy::new(
            &self.connection,
            NM_DEST,
            connection.as_str(),
            SETTINGS_CONNECTION_IFACE,
        )
        .await?;
        let mut settings: HashMap<String, PropMap> = proxy.call("GetSettings", &()).await?;
        let security_settings = settings
            .entry("802-11-wireless-security".to_string())
            .or_default();

        match security {
            Security::Wep => {
                security_settings.insert("key-mgmt".to_string(), owned_string("none"));
                security_settings.insert("wep-key0".to_string(), owned_string(password));
            }
            Security::Wpa | Security::Wpa2 => {
                security_settings.insert("key-mgmt".to_string(), owned_string("wpa-psk"));
                security_settings.insert("psk".to_string(), owned_string(password));
            }
            Security::Wpa3 => {
                security_settings.insert("key-mgmt".to_string(), owned_string("sae"));
                security_settings.insert("psk".to_string(), owned_string(password));
            }
            Security::Open | Security::Owe | Security::Enterprise => return Ok(()),
        }

        let _: () = proxy.call("Update", &(settings,)).await?;
        Ok(())
    }

    async fn delete_connection(&self, connection: &OwnedObjectPath) -> Result<(), NetworkError> {
        let proxy = Proxy::new(
            &self.connection,
            NM_DEST,
            connection.as_str(),
            SETTINGS_CONNECTION_IFACE,
        )
        .await?;
        let _: () = proxy.call("Delete", &()).await?;
        Ok(())
    }

    pub async fn disconnect(
        &self,
        active_connection: &OwnedObjectPath,
    ) -> Result<(), NetworkError> {
        let proxy = Proxy::new(&self.connection, NM_DEST, NM_PATH, NM_IFACE).await?;
        let _: () = proxy
            .call("DeactivateConnection", &(active_connection.clone(),))
            .await?;
        Ok(())
    }

    pub async fn find_saved_connection(
        &self,
        ssid: &str,
    ) -> Result<Option<OwnedObjectPath>, NetworkError> {
        let settings = Proxy::new(&self.connection, NM_DEST, SETTINGS_PATH, SETTINGS_IFACE).await?;
        let connections: Vec<OwnedObjectPath> = settings.call("ListConnections", &()).await?;

        for path in connections {
            let proxy = Proxy::new(
                &self.connection,
                NM_DEST,
                path.as_str(),
                SETTINGS_CONNECTION_IFACE,
            )
            .await?;
            let settings: HashMap<String, PropMap> = proxy.call("GetSettings", &()).await?;

            let Some(connection) = settings.get("connection") else {
                continue;
            };
            if prop::<String>(connection, "type").as_deref() != Some("802-11-wireless") {
                continue;
            }
            let Some(wireless) = settings.get("802-11-wireless") else {
                continue;
            };
            if prop::<Vec<u8>>(wireless, "ssid").is_some_and(|saved| saved == ssid.as_bytes()) {
                return Ok(Some(path));
            }
        }

        Ok(None)
    }

    pub async fn devices(&self) -> Result<Vec<DeviceInfo>, NetworkError> {
        let network_manager = Proxy::new(&self.connection, NM_DEST, NM_PATH, NM_IFACE).await?;
        let devices: Vec<OwnedObjectPath> = network_manager.call("GetDevices", &()).await?;
        let mut result = Vec::new();

        for path in devices {
            let proxy = Proxy::new(&self.connection, NM_DEST, path.as_str(), DEVICE_IFACE).await?;
            let device_type = proxy.get_property::<u32>("DeviceType").await.unwrap_or(0);
            let kind = match device_type {
                1 => DeviceKind::Ethernet,
                2 => DeviceKind::Wifi,
                _ => continue,
            };
            let interface = proxy
                .get_property::<String>("Interface")
                .await
                .unwrap_or_else(|_| path.to_string());
            result.push(DeviceInfo {
                path,
                interface,
                kind,
            });
        }

        result.sort_by(|left, right| left.interface.cmp(&right.interface));
        Ok(result)
    }

    pub async fn load_profile(&self, path: &OwnedObjectPath) -> Result<ProfileDraft, NetworkError> {
        let proxy = Proxy::new(
            &self.connection,
            NM_DEST,
            path.as_str(),
            SETTINGS_CONNECTION_IFACE,
        )
        .await?;
        let settings: HashMap<String, PropMap> = proxy.call("GetSettings", &()).await?;
        draft_from_settings(path.clone(), &settings).map_err(NetworkError::Message)
    }

    pub async fn save_profile(&self, draft: &ProfileDraft) -> Result<(), NetworkError> {
        if let Some(path) = &draft.path {
            let proxy = Proxy::new(
                &self.connection,
                NM_DEST,
                path.as_str(),
                SETTINGS_CONNECTION_IFACE,
            )
            .await?;
            let settings: HashMap<String, PropMap> = proxy.call("GetSettings", &()).await?;
            let settings =
                settings_from_draft(draft, Some(settings)).map_err(NetworkError::Message)?;
            let _: () = proxy.call("Update", &(settings,)).await?;
        } else {
            let settings = settings_from_draft(draft, None).map_err(NetworkError::Message)?;
            let settings_proxy =
                Proxy::new(&self.connection, NM_DEST, SETTINGS_PATH, SETTINGS_IFACE).await?;
            let _: OwnedObjectPath = settings_proxy.call("AddConnection", &(settings,)).await?;
        }
        Ok(())
    }

    pub async fn known_profiles(&self) -> Result<HashMap<String, KnownProfile>, NetworkError> {
        let settings = Proxy::new(&self.connection, NM_DEST, SETTINGS_PATH, SETTINGS_IFACE).await?;
        let connections: Vec<OwnedObjectPath> = settings.call("ListConnections", &()).await?;
        let mut known = HashMap::new();

        for path in connections {
            let proxy = Proxy::new(
                &self.connection,
                NM_DEST,
                path.as_str(),
                SETTINGS_CONNECTION_IFACE,
            )
            .await?;
            let settings: HashMap<String, PropMap> = proxy.call("GetSettings", &()).await?;

            let Some(connection) = settings.get("connection") else {
                continue;
            };
            if prop::<String>(connection, "type").as_deref() != Some("802-11-wireless") {
                continue;
            }
            let Some(wireless) = settings.get("802-11-wireless") else {
                continue;
            };
            if let Some(ssid) = prop::<Vec<u8>>(wireless, "ssid") {
                known.insert(
                    String::from_utf8_lossy(&ssid).into_owned(),
                    KnownProfile {
                        hidden: prop::<bool>(wireless, "hidden").unwrap_or(false),
                        auto_connect: prop::<bool>(connection, "autoconnect").unwrap_or(true),
                    },
                );
            }
        }

        Ok(known)
    }
}

async fn next_state_reason<S>(signals: &mut S) -> Option<u32>
where
    S: Stream<Item = zbus::Message> + Unpin,
{
    signals.next().await.and_then(|message| {
        message
            .body()
            .deserialize::<(u32, u32, u32)>()
            .ok()
            .map(|(_, _, reason)| reason)
    })
}

async fn device_state_reason(proxy: &Proxy<'_>) -> u32 {
    proxy
        .get_property::<(u32, u32)>("StateReason")
        .await
        .map(|(_, reason)| reason)
        .unwrap_or(0)
}

fn is_wrong_password_reason(reason: u32) -> bool {
    matches!(reason, 7 | 8 | 10 | 11)
}

fn needs_password(security: Security) -> bool {
    matches!(
        security,
        Security::Wep | Security::Wpa | Security::Wpa2 | Security::Wpa3
    )
}

fn owned_string(value: &str) -> OwnedVariant {
    OwnedVariant::try_from(Variant::from(value.to_string())).expect("string variant is owned")
}

fn activation_error(reason: u32) -> String {
    match reason {
        0 => "Connection failed".to_string(),
        2 => "Connection cancelled".to_string(),
        3 => "Device disconnected".to_string(),
        5 => "IP configuration unavailable".to_string(),
        6 => "IP configuration expired".to_string(),
        7 | 8 | 10 | 11 => "Wrong password".to_string(),
        9 => "Wi-Fi configuration failed".to_string(),
        _ => format!("Connection failed (reason {reason})"),
    }
}

fn connection_settings<'a>(
    ssid: &'a str,
    security: Security,
    password: Option<&'a str>,
) -> Result<HashMap<&'a str, HashMap<&'a str, Variant<'a>>>, NetworkError> {
    let mut settings = HashMap::new();

    let mut connection = HashMap::new();
    connection.insert("id", Variant::from(ssid));
    connection.insert("type", Variant::from("802-11-wireless"));
    settings.insert("connection", connection);

    let mut wireless = HashMap::new();
    wireless.insert("ssid", Variant::from(ssid.as_bytes().to_vec()));
    settings.insert("802-11-wireless", wireless);

    if security != Security::Open {
        let mut security_settings = HashMap::new();
        match security {
            Security::Open => {}
            Security::Wep => {
                security_settings.insert("key-mgmt", Variant::from("none"));
                if let Some(password) = password {
                    security_settings.insert("wep-key0", Variant::from(password));
                }
            }
            Security::Wpa | Security::Wpa2 => {
                security_settings.insert("key-mgmt", Variant::from("wpa-psk"));
                if let Some(password) = password {
                    security_settings.insert("psk", Variant::from(password));
                }
            }
            Security::Wpa3 => {
                security_settings.insert("key-mgmt", Variant::from("sae"));
                if let Some(password) = password {
                    security_settings.insert("psk", Variant::from(password));
                }
            }
            Security::Owe => {
                security_settings.insert("key-mgmt", Variant::from("owe"));
            }
            Security::Enterprise => {
                return Err(NetworkError::Message(
                    "Enterprise networks are not supported yet".to_string(),
                ));
            }
        }
        settings.insert("802-11-wireless-security", security_settings);
    }

    let mut ipv4 = HashMap::new();
    ipv4.insert("method", Variant::from("auto"));
    settings.insert("ipv4", ipv4);

    let mut ipv6 = HashMap::new();
    ipv6.insert("method", Variant::from("auto"));
    settings.insert("ipv6", ipv6);

    Ok(settings)
}

fn build_snapshot(objects: &ObjectMap) -> Snapshot {
    let Some(wifi_object) = objects
        .iter()
        .find(|(_, interfaces)| interfaces.contains_key(WIRELESS_IFACE))
    else {
        return Snapshot::default();
    };
    let (device_path, interfaces) = wifi_object;
    let device_props = interfaces.get(DEVICE_IFACE);
    let wireless_props = interfaces.get(WIRELESS_IFACE);
    let empty = PropMap::new();
    let device_props = device_props.unwrap_or(&empty);
    let wireless_props = wireless_props.unwrap_or(&empty);

    let interface =
        prop::<String>(device_props, "Interface").unwrap_or_else(|| "wlan0".to_string());
    let active_access_point = prop::<OwnedObjectPath>(wireless_props, "ActiveAccessPoint")
        .filter(|path| path.as_str() != "/");
    let access_points =
        prop::<Vec<OwnedObjectPath>>(wireless_props, "AccessPoints").unwrap_or_default();

    let mut networks_by_ssid: HashMap<String, WifiNetwork> = HashMap::new();
    for access_point in access_points {
        let Some(access_point_props) = objects
            .get(&access_point)
            .and_then(|interfaces| interfaces.get(ACCESS_POINT_IFACE))
        else {
            continue;
        };
        let Some((ssid, strength, frequency, security)) = parse_access_point(access_point_props)
        else {
            continue;
        };
        let network = WifiNetwork {
            active: active_access_point.as_ref() == Some(&access_point),
            ap_path: access_point,
            ssid: ssid.clone(),
            strength,
            frequency,
            security,
            known: false,
            hidden: false,
            auto_connect: false,
        };
        match networks_by_ssid.get(&ssid) {
            Some(existing)
                if (existing.active && !network.active)
                    || (existing.active == network.active
                        && existing.strength >= network.strength) => {}
            _ => {
                networks_by_ssid.insert(ssid, network);
            }
        }
    }

    let mut networks: Vec<WifiNetwork> = networks_by_ssid.into_values().collect();
    networks.sort_by(|left, right| {
        right
            .active
            .cmp(&left.active)
            .then_with(|| right.strength.cmp(&left.strength))
            .then_with(|| left.ssid.cmp(&right.ssid))
    });

    let active_connection = prop::<OwnedObjectPath>(device_props, "ActiveConnection")
        .filter(|path| path.as_str() != "/");
    let active = match (&active_access_point, active_connection) {
        (Some(access_point), Some(active_connection)) => {
            let access_point_props = objects
                .get(access_point)
                .and_then(|interfaces| interfaces.get(ACCESS_POINT_IFACE));
            access_point_props.and_then(parse_access_point).map(
                |(ssid, strength, frequency, security)| ActiveWifi {
                    ip: ipv4_address(objects, &active_connection),
                    active_path: active_connection,
                    ssid,
                    strength,
                    frequency,
                    security,
                },
            )
        }
        _ => None,
    };

    Snapshot {
        wifi: Some(WifiState {
            device_path: device_path.clone(),
            interface,
            active,
            networks,
        }),
    }
}

fn parse_access_point(props: &PropMap) -> Option<(String, u8, u32, Security)> {
    let ssid = prop::<Vec<u8>>(props, "Ssid")?;
    if ssid.is_empty() {
        return None;
    }
    Some((
        String::from_utf8_lossy(&ssid).into_owned(),
        prop::<u8>(props, "Strength").unwrap_or(0),
        prop::<u32>(props, "Frequency").unwrap_or(0),
        security_from_flags(
            prop::<u32>(props, "Flags").unwrap_or(0),
            prop::<u32>(props, "WpaFlags").unwrap_or(0),
            prop::<u32>(props, "RsnFlags").unwrap_or(0),
        ),
    ))
}

fn security_from_flags(flags: u32, wpa_flags: u32, rsn_flags: u32) -> Security {
    const PRIVACY: u32 = 0x1;
    const PAIR_WEP40: u32 = 0x1;
    const PAIR_WEP104: u32 = 0x2;
    const GROUP_WEP40: u32 = 0x10;
    const GROUP_WEP104: u32 = 0x20;
    const KEY_MGMT_PSK: u32 = 0x100;
    const KEY_MGMT_802_1X: u32 = 0x200;
    const KEY_MGMT_SAE: u32 = 0x400;
    const KEY_MGMT_OWE: u32 = 0x800;
    const KEY_MGMT_OWE_TM: u32 = 0x1000;
    const KEY_MGMT_EAP_SUITE_B_192: u32 = 0x2000;
    const WEP_FLAGS: u32 = PAIR_WEP40 | PAIR_WEP104 | GROUP_WEP40 | GROUP_WEP104;

    let key_management = wpa_flags | rsn_flags;
    if key_management & (KEY_MGMT_OWE | KEY_MGMT_OWE_TM) != 0 {
        Security::Owe
    } else if key_management & KEY_MGMT_SAE != 0 {
        Security::Wpa3
    } else if key_management & (KEY_MGMT_802_1X | KEY_MGMT_EAP_SUITE_B_192) != 0 {
        Security::Enterprise
    } else if key_management & KEY_MGMT_PSK != 0 {
        if rsn_flags & KEY_MGMT_PSK != 0 {
            Security::Wpa2
        } else {
            Security::Wpa
        }
    } else if flags & PRIVACY != 0 || key_management & WEP_FLAGS != 0 {
        Security::Wep
    } else {
        Security::Open
    }
}

fn ipv4_address(objects: &ObjectMap, active_connection: &OwnedObjectPath) -> Option<String> {
    let active_props = objects
        .get(active_connection)
        .and_then(|interfaces| interfaces.get(ACTIVE_CONNECTION_IFACE))?;
    let ip4_path =
        prop::<OwnedObjectPath>(active_props, "Ip4Config").filter(|path| path.as_str() != "/")?;
    let ip4_props = objects
        .get(&ip4_path)
        .and_then(|interfaces| interfaces.get(IP4_CONFIG_IFACE))?;
    let addresses = prop::<Vec<HashMap<String, OwnedVariant>>>(ip4_props, "AddressData")?;
    let entry = addresses.into_iter().next()?;
    let address = prop::<String>(&entry, "address")?;
    let prefix = prop::<u32>(&entry, "prefix").unwrap_or(0);
    Some(format!("{address}/{prefix}"))
}

fn prop<T>(props: &PropMap, key: &str) -> Option<T>
where
    T: TryFrom<OwnedVariant>,
{
    props.get(key)?.try_clone().ok()?.try_into().ok()
}
