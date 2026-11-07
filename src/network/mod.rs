mod client;
mod profile;
mod types;

pub use client::{ConnectOutcome, NMClient, NetworkError};
pub use profile::{
    ConnectionType, DeviceInfo, DeviceKind, EthernetConfig, IpAddress, IpConfig, IpMethod,
    ProfileDraft, WifiConfig, WifiMode, parse_addresses, parse_ip_list, parse_search_list,
};
pub use types::{
    ActiveWifi, KnownProfile, Security, Snapshot, WifiNetwork, WifiState, frequency_label,
};
