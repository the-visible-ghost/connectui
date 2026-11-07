use crate::network::{
    ConnectionType, DeviceInfo, DeviceKind, IpMethod, ProfileDraft, Security, WifiMode,
    parse_addresses, parse_ip_list, parse_search_list,
};
use crate::ui::common::Theme;
use iocraft::components::{ScrollView, ScrollViewHandle};
use iocraft::hooks::Ref;
use iocraft::prelude::*;

const LABEL_WIDTH: u32 = 22;
const COMPACT_LABEL_WIDTH: u32 = 18;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorCategory {
    Network,
    Ipv4,
    Ipv6,
}

impl EditorCategory {
    const ALL: [Self; 3] = [Self::Network, Self::Ipv4, Self::Ipv6];

    fn label(self) -> &'static str {
        match self {
            Self::Network => "Network",
            Self::Ipv4 => "IPv4",
            Self::Ipv6 => "IPv6",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Network => "󰖩",
            Self::Ipv4 => "󰩠",
            Self::Ipv6 => "󰩫",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Network => "General profile, interface and wireless settings",
            Self::Ipv4 => "Addressing, routing and DNS for IPv4",
            Self::Ipv6 => "Addressing, routing and DNS for IPv6",
        }
    }

    fn next(self, delta: isize) -> Self {
        let index = Self::ALL
            .iter()
            .position(|category| *category == self)
            .unwrap_or(0);
        Self::ALL[(index as isize + delta).rem_euclid(Self::ALL.len() as isize) as usize]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Field {
    ConnectionName,
    Device,
    Autoconnect,
    AllUsers,
    Ssid,
    WifiMode,
    Security,
    Password,
    Bssid,
    WifiClonedMac,
    WifiMtu,
    Hidden,
    EthernetMac,
    EthernetClonedMac,
    EthernetMtu,
    Ipv4Method,
    Ipv4Addresses,
    Ipv4Gateway,
    Ipv4Dns,
    Ipv4Search,
    Ipv6Method,
    Ipv6Addresses,
    Ipv6Gateway,
    Ipv6Dns,
    Ipv6Search,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Self::ConnectionName => "Profile Name",
            Self::Device => "Device",
            Self::Autoconnect => "Auto Connect",
            Self::AllUsers => "All Users",
            Self::Ssid => "SSID",
            Self::WifiMode => "Mode",
            Self::Security => "Security",
            Self::Password => "Password",
            Self::Bssid => "BSSID",
            Self::WifiClonedMac => "Cloned MAC",
            Self::WifiMtu => "MTU",
            Self::Hidden => "Hidden",
            Self::EthernetMac => "MAC Address",
            Self::EthernetClonedMac => "Cloned MAC",
            Self::EthernetMtu => "MTU",
            Self::Ipv4Method => "Method",
            Self::Ipv4Addresses => "Addresses",
            Self::Ipv4Gateway => "Gateway",
            Self::Ipv4Dns => "DNS Servers",
            Self::Ipv4Search => "Search Domains",
            Self::Ipv6Method => "Method",
            Self::Ipv6Addresses => "Addresses",
            Self::Ipv6Gateway => "Gateway",
            Self::Ipv6Dns => "DNS Servers",
            Self::Ipv6Search => "Search Domains",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::ConnectionName => "Name NetworkManager uses for this profile",
            Self::Device => "Restrict this profile to one interface",
            Self::Autoconnect => "Connect automatically when the device is available",
            Self::AllUsers => "Allow every user to use and change this profile",
            Self::Ssid => "Wireless network name",
            Self::WifiMode => "Wireless operating mode",
            Self::Security => "Wireless authentication method",
            Self::Password => "Leave blank to keep the saved password",
            Self::Bssid => "Optional access point MAC address",
            Self::WifiClonedMac => "Optional MAC address presented to the network",
            Self::WifiMtu => "Optional maximum transmission unit",
            Self::Hidden => "Connect to a network that does not broadcast its SSID",
            Self::EthernetMac => "Optional permanent MAC address",
            Self::EthernetClonedMac => "Optional MAC address presented to the network",
            Self::EthernetMtu => "Optional maximum transmission unit",
            Self::Ipv4Method | Self::Ipv6Method => "Select how addresses are assigned",
            Self::Ipv4Addresses => "Comma-separated addresses in address/prefix form",
            Self::Ipv6Addresses => "Comma-separated addresses in address/prefix form",
            Self::Ipv4Gateway | Self::Ipv6Gateway => "Optional default gateway",
            Self::Ipv4Dns | Self::Ipv6Dns => "Comma-separated DNS server addresses",
            Self::Ipv4Search | Self::Ipv6Search => "Comma-separated search domains",
        }
    }

    fn is_toggle(self) -> bool {
        matches!(self, Self::Autoconnect | Self::AllUsers | Self::Hidden)
    }

    fn is_select(self) -> bool {
        matches!(
            self,
            Self::Device | Self::WifiMode | Self::Security | Self::Ipv4Method | Self::Ipv6Method
        )
    }

    fn is_password(self) -> bool {
        self == Self::Password
    }
}

#[derive(Clone)]
pub struct InputState {
    field: Field,
    value: String,
    cursor: usize,
}

#[derive(Clone)]
pub struct SelectState {
    field: Field,
    selected: usize,
}

#[derive(Clone)]
pub struct EditorState {
    pub draft: ProfileDraft,
    pub original: ProfileDraft,
    pub is_new: bool,
    pub devices: Vec<DeviceInfo>,
    pub category: EditorCategory,
    pub focus: usize,
    pub input: Option<InputState>,
    pub select: Option<SelectState>,
    pub confirm: bool,
    pub saving: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
pub enum EditorAction {
    None,
    Close,
    Save(Box<ProfileDraft>),
}

impl EditorState {
    pub fn new(draft: ProfileDraft, devices: Vec<DeviceInfo>, is_new: bool) -> Self {
        Self {
            original: draft.clone(),
            draft,
            is_new,
            devices,
            category: EditorCategory::Network,
            focus: 0,
            input: None,
            select: None,
            confirm: false,
            saving: false,
            error: None,
        }
    }

    pub fn dirty(&self) -> bool {
        self.is_new || self.draft != self.original
    }

    pub fn selected_row(&self) -> usize {
        self.focus
    }

    fn fields(&self) -> Vec<Field> {
        let mut fields = Vec::new();
        match self.category {
            EditorCategory::Network => {
                fields.extend([
                    Field::ConnectionName,
                    Field::Device,
                    Field::Autoconnect,
                    Field::AllUsers,
                ]);
                match self.draft.connection_type {
                    ConnectionType::Wifi => {
                        fields.push(Field::Ssid);
                        fields.push(Field::WifiMode);
                        fields.push(Field::Security);
                        if self
                            .draft
                            .wifi
                            .as_ref()
                            .is_some_and(|wifi| needs_password(wifi.security))
                        {
                            fields.push(Field::Password);
                        }
                        fields.extend([
                            Field::Bssid,
                            Field::WifiClonedMac,
                            Field::WifiMtu,
                            Field::Hidden,
                        ]);
                    }
                    ConnectionType::Ethernet => {
                        fields.extend([
                            Field::EthernetMac,
                            Field::EthernetClonedMac,
                            Field::EthernetMtu,
                        ]);
                    }
                }
            }
            EditorCategory::Ipv4 => {
                fields.push(Field::Ipv4Method);
                if self.draft.ipv4.method == IpMethod::Manual {
                    fields.push(Field::Ipv4Addresses);
                    fields.push(Field::Ipv4Gateway);
                }
                if self.draft.ipv4.method != IpMethod::Disabled {
                    fields.push(Field::Ipv4Dns);
                    fields.push(Field::Ipv4Search);
                }
            }
            EditorCategory::Ipv6 => {
                fields.push(Field::Ipv6Method);
                if self.draft.ipv6.method == IpMethod::Manual {
                    fields.push(Field::Ipv6Addresses);
                    fields.push(Field::Ipv6Gateway);
                }
                if self.draft.ipv6.method != IpMethod::Disabled {
                    fields.push(Field::Ipv6Dns);
                    fields.push(Field::Ipv6Search);
                }
            }
        }
        fields
    }

    fn current_field(&self) -> Field {
        let fields = self.fields();
        fields[self.focus.min(fields.len().saturating_sub(1))]
    }

    fn clamp_focus(&mut self) {
        let count = self.fields().len();
        if count == 0 {
            self.focus = 0;
        } else {
            self.focus = self.focus.min(count - 1);
        }
    }

    fn selected_device_kind(&self) -> DeviceKind {
        match self.draft.connection_type {
            ConnectionType::Wifi => DeviceKind::Wifi,
            ConnectionType::Ethernet => DeviceKind::Ethernet,
        }
    }

    fn compatible_devices(&self) -> Vec<&DeviceInfo> {
        let kind = self.selected_device_kind();
        self.devices
            .iter()
            .filter(|device| device.kind == kind)
            .collect()
    }
}

pub fn handle_key(editor: &mut EditorState, code: KeyCode) -> EditorAction {
    if editor.saving {
        return EditorAction::None;
    }

    if editor.confirm {
        match code {
            KeyCode::Enter | KeyCode::Char('s') => {
                editor.saving = true;
                editor.confirm = false;
                return EditorAction::Save(Box::new(editor.draft.clone()));
            }
            KeyCode::Char('d') => return EditorAction::Close,
            KeyCode::Esc => editor.confirm = false,
            _ => {}
        }
        return EditorAction::None;
    }

    if editor.select.is_some() {
        handle_select_key(editor, code);
        return EditorAction::None;
    }

    if editor.input.is_some() {
        handle_input_key(editor, code);
        return EditorAction::None;
    }

    match code {
        KeyCode::Esc => {
            if editor.dirty() {
                editor.confirm = true;
            } else {
                return EditorAction::Close;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let count = editor.fields().len();
            if count > 0 {
                editor.focus = (editor.focus as isize - 1).rem_euclid(count as isize) as usize;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let count = editor.fields().len();
            if count > 0 {
                editor.focus = (editor.focus as isize + 1).rem_euclid(count as isize) as usize;
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            editor.category = editor.category.next(-1);
            editor.focus = 0;
        }
        KeyCode::Right | KeyCode::Char('l') => {
            editor.category = editor.category.next(1);
            editor.focus = 0;
        }
        KeyCode::Tab => {
            editor.category = editor.category.next(1);
            editor.focus = 0;
        }
        KeyCode::BackTab => {
            editor.category = editor.category.next(-1);
            editor.focus = 0;
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            let field = editor.current_field();
            activate_field(editor, field);
        }
        _ => {}
    }

    EditorAction::None
}

fn handle_select_key(editor: &mut EditorState, code: KeyCode) {
    let Some(select) = editor.select.as_ref() else {
        return;
    };
    let field = select.field;
    let mut selected = select.selected;
    let count = select_options(editor, field).len();
    match code {
        KeyCode::Esc => editor.select = None,
        KeyCode::Up | KeyCode::Char('k') if count > 0 => {
            selected = (selected as isize - 1).rem_euclid(count as isize) as usize;
            if let Some(select) = editor.select.as_mut() {
                select.selected = selected;
            }
        }
        KeyCode::Down | KeyCode::Char('j') if count > 0 => {
            selected = (selected as isize + 1).rem_euclid(count as isize) as usize;
            if let Some(select) = editor.select.as_mut() {
                select.selected = selected;
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            editor.select = None;
            apply_select(editor, field, selected);
        }
        _ => {}
    }
}

fn handle_input_key(editor: &mut EditorState, code: KeyCode) {
    let Some(input) = editor.input.as_mut() else {
        return;
    };
    match code {
        KeyCode::Esc => editor.input = None,
        KeyCode::Enter => {
            let input = editor.input.take().expect("input exists");
            match commit_input(editor, input.field, &input.value) {
                Ok(()) => editor.error = None,
                Err(message) => {
                    editor.error = Some(message);
                    let cursor = input.value.len();
                    editor.input = Some(InputState {
                        field: input.field,
                        value: input.value,
                        cursor,
                    });
                }
            }
            editor.clamp_focus();
        }
        KeyCode::Backspace => {
            if input.cursor > 0 {
                let previous = previous_boundary(&input.value, input.cursor);
                input.value.drain(previous..input.cursor);
                input.cursor = previous;
            }
        }
        KeyCode::Delete => {
            if input.cursor < input.value.len() {
                let next = next_boundary(&input.value, input.cursor);
                input.value.drain(input.cursor..next);
            }
        }
        KeyCode::Left => input.cursor = previous_boundary(&input.value, input.cursor),
        KeyCode::Right => input.cursor = next_boundary(&input.value, input.cursor),
        KeyCode::Home => input.cursor = 0,
        KeyCode::End => input.cursor = input.value.len(),
        KeyCode::Char(character) => {
            input.value.insert(input.cursor, character);
            input.cursor += character.len_utf8();
        }
        _ => {}
    }
}

fn activate_field(editor: &mut EditorState, field: Field) {
    if field.is_toggle() {
        toggle_field(editor, field);
        editor.error = None;
        return;
    }
    if field.is_select() {
        let selected = select_index(editor, field);
        editor.select = Some(SelectState { field, selected });
        return;
    }
    let value = initial_text(editor, field);
    let cursor = value.len();
    editor.input = Some(InputState {
        field,
        value,
        cursor,
    });
    editor.error = None;
}

fn toggle_field(editor: &mut EditorState, field: Field) {
    match field {
        Field::Autoconnect => editor.draft.autoconnect = !editor.draft.autoconnect,
        Field::AllUsers => editor.draft.all_users = !editor.draft.all_users,
        Field::Hidden => {
            if let Some(wifi) = editor.draft.wifi.as_mut() {
                wifi.hidden = !wifi.hidden;
            }
        }
        _ => {}
    }
}

fn initial_text(editor: &EditorState, field: Field) -> String {
    match field {
        Field::ConnectionName => editor.draft.id.clone(),
        Field::Ssid => editor
            .draft
            .wifi
            .as_ref()
            .map(|wifi| wifi.ssid.clone())
            .unwrap_or_default(),
        Field::Password => String::new(),
        Field::Bssid => editor
            .draft
            .wifi
            .as_ref()
            .map(|wifi| wifi.bssid.clone())
            .unwrap_or_default(),
        Field::WifiClonedMac => editor
            .draft
            .wifi
            .as_ref()
            .map(|wifi| wifi.cloned_mac_address.clone())
            .unwrap_or_default(),
        Field::WifiMtu => editor
            .draft
            .wifi
            .as_ref()
            .and_then(|wifi| wifi.mtu)
            .map(|value| value.to_string())
            .unwrap_or_default(),
        Field::EthernetMac => editor
            .draft
            .ethernet
            .as_ref()
            .map(|ethernet| ethernet.mac_address.clone())
            .unwrap_or_default(),
        Field::EthernetClonedMac => editor
            .draft
            .ethernet
            .as_ref()
            .map(|ethernet| ethernet.cloned_mac_address.clone())
            .unwrap_or_default(),
        Field::EthernetMtu => editor
            .draft
            .ethernet
            .as_ref()
            .and_then(|ethernet| ethernet.mtu)
            .map(|value| value.to_string())
            .unwrap_or_default(),
        Field::Ipv4Addresses => editor.draft.ipv4.addresses_text(),
        Field::Ipv4Gateway => editor.draft.ipv4.gateway.clone(),
        Field::Ipv4Dns => editor.draft.ipv4.dns_text(),
        Field::Ipv4Search => editor.draft.ipv4.search_text(),
        Field::Ipv6Addresses => editor.draft.ipv6.addresses_text(),
        Field::Ipv6Gateway => editor.draft.ipv6.gateway.clone(),
        Field::Ipv6Dns => editor.draft.ipv6.dns_text(),
        Field::Ipv6Search => editor.draft.ipv6.search_text(),
        _ => String::new(),
    }
}

fn commit_input(editor: &mut EditorState, field: Field, value: &str) -> Result<(), String> {
    match field {
        Field::ConnectionName => {
            let value = value.trim();
            if value.is_empty() {
                return Err("Profile name cannot be empty".to_string());
            }
            editor.draft.id = value.to_string();
        }
        Field::Ssid => {
            let value = value.trim();
            if value.is_empty() {
                return Err("SSID cannot be empty".to_string());
            }
            if let Some(wifi) = editor.draft.wifi.as_mut() {
                wifi.ssid = value.to_string();
            }
        }
        Field::Password => {
            if let Some(wifi) = editor.draft.wifi.as_mut()
                && !value.is_empty()
            {
                wifi.password = Some(value.to_string());
                wifi.password_set = true;
            }
        }
        Field::Bssid => {
            if let Some(wifi) = editor.draft.wifi.as_mut() {
                wifi.bssid = value.trim().to_string();
            }
        }
        Field::WifiClonedMac => {
            if let Some(wifi) = editor.draft.wifi.as_mut() {
                wifi.cloned_mac_address = value.trim().to_string();
            }
        }
        Field::WifiMtu => {
            let mtu = parse_mtu(value)?;
            if let Some(wifi) = editor.draft.wifi.as_mut() {
                wifi.mtu = mtu;
            }
        }
        Field::EthernetMac => {
            if let Some(ethernet) = editor.draft.ethernet.as_mut() {
                ethernet.mac_address = value.trim().to_string();
            }
        }
        Field::EthernetClonedMac => {
            if let Some(ethernet) = editor.draft.ethernet.as_mut() {
                ethernet.cloned_mac_address = value.trim().to_string();
            }
        }
        Field::EthernetMtu => {
            let mtu = parse_mtu(value)?;
            if let Some(ethernet) = editor.draft.ethernet.as_mut() {
                ethernet.mtu = mtu;
            }
        }
        Field::Ipv4Addresses => {
            editor.draft.ipv4.addresses = parse_addresses(value, false)?;
        }
        Field::Ipv4Gateway => editor.draft.ipv4.gateway = value.trim().to_string(),
        Field::Ipv4Dns => editor.draft.ipv4.dns = parse_ip_list(value, false)?,
        Field::Ipv4Search => editor.draft.ipv4.search = parse_search_list(value),
        Field::Ipv6Addresses => {
            editor.draft.ipv6.addresses = parse_addresses(value, true)?;
        }
        Field::Ipv6Gateway => editor.draft.ipv6.gateway = value.trim().to_string(),
        Field::Ipv6Dns => editor.draft.ipv6.dns = parse_ip_list(value, true)?,
        Field::Ipv6Search => editor.draft.ipv6.search = parse_search_list(value),
        _ => {}
    }
    Ok(())
}

fn parse_mtu(value: &str) -> Result<Option<u32>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let mtu = value
        .parse::<u32>()
        .map_err(|_| "MTU must be a number".to_string())?;
    if mtu == 0 {
        return Err("MTU must be greater than zero".to_string());
    }
    Ok(Some(mtu))
}

fn select_options(editor: &EditorState, field: Field) -> Vec<String> {
    match field {
        Field::Device => {
            let mut options = vec!["Any device".to_string()];
            options.extend(
                editor
                    .compatible_devices()
                    .into_iter()
                    .map(|device| device.interface.clone()),
            );
            let interface = &editor.draft.interface_name;
            if !interface.is_empty() && !options.iter().any(|option| option == interface) {
                options.push(interface.clone());
            }
            options
        }
        Field::Security => {
            let mut options = vec![
                Security::Open,
                Security::Wep,
                Security::Wpa,
                Security::Wpa2,
                Security::Wpa3,
                Security::Owe,
            ]
            .into_iter()
            .map(|security| security.label().to_string())
            .collect::<Vec<_>>();
            if editor
                .draft
                .wifi
                .as_ref()
                .is_some_and(|wifi| wifi.security == Security::Enterprise)
            {
                options.push(Security::Enterprise.label().to_string());
            }
            options
        }
        Field::WifiMode => {
            let mut options = vec![
                WifiMode::Infrastructure.label(),
                WifiMode::Adhoc.label(),
                WifiMode::Ap.label(),
            ];
            if let Some(WifiMode::Custom(value)) = editor.draft.wifi.as_ref().map(|wifi| &wifi.mode)
            {
                options.push(value.clone());
            }
            options
        }
        Field::Ipv4Method | Field::Ipv6Method => IpMethod::OPTIONS
            .into_iter()
            .map(|method| method.label().to_string())
            .collect(),
        _ => Vec::new(),
    }
}

fn select_index(editor: &EditorState, field: Field) -> usize {
    match field {
        Field::Device => {
            let interface = &editor.draft.interface_name;
            if interface.is_empty() {
                0
            } else {
                select_options(editor, field)
                    .iter()
                    .position(|option| option == interface)
                    .unwrap_or(0)
            }
        }
        Field::Security => {
            let security = editor
                .draft
                .wifi
                .as_ref()
                .map(|wifi| wifi.security)
                .unwrap_or(Security::Open);
            match security {
                Security::Open => 0,
                Security::Wep => 1,
                Security::Wpa => 2,
                Security::Wpa2 => 3,
                Security::Wpa3 => 4,
                Security::Owe => 5,
                Security::Enterprise => 6,
            }
        }
        Field::WifiMode => match editor.draft.wifi.as_ref().map(|wifi| &wifi.mode) {
            Some(WifiMode::Infrastructure) => 0,
            Some(WifiMode::Adhoc) => 1,
            Some(WifiMode::Ap) => 2,
            Some(WifiMode::Custom(_)) => 3,
            None => 0,
        },
        Field::Ipv4Method => method_index(editor.draft.ipv4.method),
        Field::Ipv6Method => method_index(editor.draft.ipv6.method),
        _ => 0,
    }
}

fn method_index(method: IpMethod) -> usize {
    IpMethod::OPTIONS
        .iter()
        .position(|option| *option == method)
        .unwrap_or(0)
}

fn apply_select(editor: &mut EditorState, field: Field, selected: usize) {
    match field {
        Field::Device => {
            let options = select_options(editor, field);
            editor.draft.interface_name = if selected == 0 {
                String::new()
            } else {
                options.get(selected).cloned().unwrap_or_default()
            };
        }
        Field::Security => {
            let security = match selected {
                1 => Security::Wep,
                2 => Security::Wpa,
                3 => Security::Wpa2,
                4 => Security::Wpa3,
                5 => Security::Owe,
                6 => Security::Enterprise,
                _ => Security::Open,
            };
            if let Some(wifi) = editor.draft.wifi.as_mut() {
                wifi.security = security;
            }
            editor.clamp_focus();
        }
        Field::WifiMode => {
            let mode = match selected {
                1 => WifiMode::Adhoc,
                2 => WifiMode::Ap,
                3 => editor
                    .draft
                    .wifi
                    .as_ref()
                    .map(|wifi| wifi.mode.clone())
                    .filter(|mode| matches!(mode, WifiMode::Custom(_)))
                    .unwrap_or(WifiMode::Infrastructure),
                _ => WifiMode::Infrastructure,
            };
            if let Some(wifi) = editor.draft.wifi.as_mut() {
                wifi.mode = mode;
            }
        }
        Field::Ipv4Method => {
            editor.draft.ipv4.method = IpMethod::OPTIONS[selected.min(4)];
            editor.clamp_focus();
        }
        Field::Ipv6Method => {
            editor.draft.ipv6.method = IpMethod::OPTIONS[selected.min(4)];
            editor.clamp_focus();
        }
        _ => {}
    }
    editor.error = None;
}

fn needs_password(security: Security) -> bool {
    matches!(
        security,
        Security::Wep | Security::Wpa | Security::Wpa2 | Security::Wpa3
    )
}

fn previous_boundary(value: &str, cursor: usize) -> usize {
    value[..cursor]
        .char_indices()
        .next_back()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn next_boundary(value: &str, cursor: usize) -> usize {
    value[cursor..]
        .chars()
        .next()
        .map(|character| cursor + character.len_utf8())
        .unwrap_or(cursor)
}

fn display_value(editor: &EditorState, field: Field) -> String {
    if let Some(input) = editor.input.as_ref().filter(|input| input.field == field) {
        return if field.is_password() {
            format!(
                "{}{}",
                "*".repeat(input.value.chars().count()),
                cursor_mark()
            )
        } else {
            let mut value = input.value.clone();
            value.insert_str(input.cursor, cursor_mark());
            value
        };
    }

    match field {
        Field::ConnectionName => editor.draft.id.clone(),
        Field::Device => {
            if editor.draft.interface_name.is_empty() {
                "Any device".to_string()
            } else {
                editor.draft.interface_name.clone()
            }
        }
        Field::Autoconnect => yes_no(editor.draft.autoconnect),
        Field::AllUsers => yes_no(editor.draft.all_users),
        Field::Ssid => editor
            .draft
            .wifi
            .as_ref()
            .map(|wifi| wifi.ssid.clone())
            .unwrap_or_default(),
        Field::WifiMode => editor
            .draft
            .wifi
            .as_ref()
            .map(|wifi| wifi.mode.label())
            .unwrap_or_default(),
        Field::Security => editor
            .draft
            .wifi
            .as_ref()
            .map(|wifi| wifi.security.label().to_string())
            .unwrap_or_default(),
        Field::Password => editor
            .draft
            .wifi
            .as_ref()
            .map(|wifi| {
                if wifi.password_set {
                    "••••••••".to_string()
                } else {
                    "Not set".to_string()
                }
            })
            .unwrap_or_default(),
        Field::Bssid => optional_value(
            editor
                .draft
                .wifi
                .as_ref()
                .map(|wifi| wifi.bssid.as_str())
                .unwrap_or(""),
            "Automatic",
        ),
        Field::WifiClonedMac => optional_value(
            editor
                .draft
                .wifi
                .as_ref()
                .map(|wifi| wifi.cloned_mac_address.as_str())
                .unwrap_or(""),
            "None",
        ),
        Field::WifiMtu => editor
            .draft
            .wifi
            .as_ref()
            .and_then(|wifi| wifi.mtu)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "Automatic".to_string()),
        Field::Hidden => yes_no(editor.draft.wifi.as_ref().is_some_and(|wifi| wifi.hidden)),
        Field::EthernetMac => optional_value(
            editor
                .draft
                .ethernet
                .as_ref()
                .map(|ethernet| ethernet.mac_address.as_str())
                .unwrap_or(""),
            "Permanent",
        ),
        Field::EthernetClonedMac => optional_value(
            editor
                .draft
                .ethernet
                .as_ref()
                .map(|ethernet| ethernet.cloned_mac_address.as_str())
                .unwrap_or(""),
            "None",
        ),
        Field::EthernetMtu => editor
            .draft
            .ethernet
            .as_ref()
            .and_then(|ethernet| ethernet.mtu)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "Automatic".to_string()),
        Field::Ipv4Method => editor.draft.ipv4.method.label().to_string(),
        Field::Ipv4Addresses => optional_value(&editor.draft.ipv4.addresses_text(), "None"),
        Field::Ipv4Gateway => optional_value(&editor.draft.ipv4.gateway, "None"),
        Field::Ipv4Dns => optional_value(&editor.draft.ipv4.dns_text(), "Automatic"),
        Field::Ipv4Search => optional_value(&editor.draft.ipv4.search_text(), "None"),
        Field::Ipv6Method => editor.draft.ipv6.method.label().to_string(),
        Field::Ipv6Addresses => optional_value(&editor.draft.ipv6.addresses_text(), "None"),
        Field::Ipv6Gateway => optional_value(&editor.draft.ipv6.gateway, "None"),
        Field::Ipv6Dns => optional_value(&editor.draft.ipv6.dns_text(), "Automatic"),
        Field::Ipv6Search => optional_value(&editor.draft.ipv6.search_text(), "None"),
    }
}

fn yes_no(value: bool) -> String {
    if value {
        "Yes".to_string()
    } else {
        "No".to_string()
    }
}

fn optional_value(value: &str, placeholder: &str) -> String {
    if value.trim().is_empty() {
        placeholder.to_string()
    } else {
        value.to_string()
    }
}

fn cursor_mark() -> &'static str {
    "▏"
}

fn field_color(theme: Theme, editor: &EditorState, field: Field) -> Color {
    let value = display_value(editor, field);
    match field {
        Field::Autoconnect | Field::AllUsers | Field::Hidden => {
            if value == "Yes" {
                Color::Green
            } else {
                Color::Grey
            }
        }
        Field::Password => {
            if value == "Not set" {
                Color::Grey
            } else {
                Color::White
            }
        }
        Field::Security => editor
            .draft
            .wifi
            .as_ref()
            .map(|wifi| match wifi.security {
                Security::Open => Color::Grey,
                Security::Wep | Security::Wpa => Color::Yellow,
                Security::Wpa2 => Color::Green,
                Security::Wpa3 | Security::Owe => Color::Cyan,
                Security::Enterprise => Color::Magenta,
            })
            .unwrap_or(theme.secondary),
        Field::Device => {
            if editor.draft.interface_name.is_empty() {
                Color::Grey
            } else {
                Color::Cyan
            }
        }
        _ => {
            if value == "None" || value == "Automatic" || value == "Permanent" {
                Color::Grey
            } else {
                Color::White
            }
        }
    }
}

fn truncate(value: &str, width: usize) -> String {
    let width = width.max(1);
    if value.chars().count() <= width {
        return value.to_string();
    }
    let mut value: String = value.chars().take(width - 1).collect();
    value.push('…');
    value
}

fn category_item(
    theme: Theme,
    category: EditorCategory,
    active: bool,
    width: u32,
) -> AnyElement<'static> {
    let color = if active { theme.primary } else { Color::Grey };
    element! {
        View(
            width: 100pct,
            height: 1,
            padding_left: 1,
            padding_right: 1,
            background_color: if active { Some(Color::DarkGrey) } else { None },
        ) {
            View(width: 2) {
                Text(
                    content: if active { "▌" } else { " " },
                    color: theme.primary,
                    weight: Weight::Bold,
                )
            }
            Text(
                content: format!("{}  {}", category.icon(), truncate(category.label(), width.saturating_sub(5) as usize)),
                color,
                weight: if active { Weight::Bold } else { Weight::Normal },
                wrap: TextWrap::NoWrap,
            )
        }
    }
    .into_any()
}

fn field_row(
    theme: Theme,
    editor: &EditorState,
    field: Field,
    selected: bool,
    label_width: u32,
) -> AnyElement<'static> {
    let value = display_value(editor, field);
    let color = field_color(theme, editor, field);
    let editing = editor
        .input
        .as_ref()
        .is_some_and(|input| input.field == field);
    let suffix = if field.is_select() {
        "󰅂"
    } else if field.is_toggle() {
        if value == "Yes" { "◉" } else { "○" }
    } else {
        ""
    };
    let value_color = if editing { theme.primary } else { color };
    let background = if selected {
        Some(Color::DarkGrey)
    } else {
        None
    };

    element! {
        View(
            width: 100pct,
            height: 1,
            padding_left: 1,
            padding_right: 1,
            background_color: background,
        ) {
            View(width: 2) {
                Text(
                    content: if selected { "▌" } else { " " },
                    color: theme.primary,
                    weight: Weight::Bold,
                )
            }
            View(width: label_width, overflow: Overflow::Hidden) {
                Text(
                    content: field.label().to_string(),
                    color: if selected { Color::White } else { Color::Grey },
                    weight: if selected { Weight::Bold } else { Weight::Normal },
                    wrap: TextWrap::NoWrap,
                )
            }
            View(flex_grow: 1.0, overflow: Overflow::Hidden) {
                Text(content: value, color: value_color, wrap: TextWrap::NoWrap)
            }
            View(width: 2, justify_content: JustifyContent::FlexEnd) {
                Text(
                    content: suffix.to_string(),
                    color: if selected { theme.primary } else { Color::DarkGrey },
                )
            }
        }
    }
    .into_any()
}

pub fn editor_panel(
    theme: Theme,
    editor: &EditorState,
    width: u32,
    handle: Ref<ScrollViewHandle>,
) -> AnyElement<'static> {
    let fields = editor.fields();
    let selected = editor.focus.min(fields.len().saturating_sub(1));
    let label_width = if width < 100 {
        COMPACT_LABEL_WIDTH
    } else {
        LABEL_WIDTH
    };
    let sidebar_width = if width < 100 { 18 } else { 21 };
    let categories: Vec<AnyElement<'static>> = EditorCategory::ALL
        .into_iter()
        .map(|category| category_item(theme, category, category == editor.category, sidebar_width))
        .collect();
    let rows: Vec<AnyElement<'static>> = fields
        .iter()
        .enumerate()
        .map(|(index, field)| field_row(theme, editor, *field, index == selected, label_width))
        .collect();
    let profile_kind = editor.draft.connection_type.label();
    let profile_id = truncate(&editor.draft.id, width.saturating_sub(32) as usize);
    let hint = editor
        .error
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(|| editor.current_field().hint().to_string());
    let hint_color = if editor.error.is_some() {
        Color::Red
    } else {
        Color::Grey
    };
    let saving = editor.saving.then(|| {
        element! {
            Text(
                content: "Saving…",
                color: theme.secondary,
                weight: Weight::Bold,
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
            border_color: theme.primary,
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
                    content: "EDIT PROFILE",
                    color: theme.primary,
                    weight: Weight::Bold,
                )
            }
            View(
                top: -1,
                right: 1,
                background_color: theme.background,
                position: Position::Absolute,
                padding_left: 1,
                padding_right: 1,
            ) {
                Text(
                    content: if editor.is_new { "NEW PROFILE" } else { "SAVED PROFILE" },
                    color: if editor.is_new { theme.secondary } else { theme.primary },
                    weight: Weight::Bold,
                )
            }

            View(
                width: 100pct,
                height: 1,
                padding_left: 1,
                padding_right: 1,
                gap: 1,
                align_items: AlignItems::Center,
            ) {
                Text(
                    content: format!("{profile_kind}  ·  {profile_id}"),
                    color: Color::Grey,
                    wrap: TextWrap::NoWrap,
                )
                View(flex_grow: 1.0)
                #(saving)
            }

            View(
                width: 100pct,
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                margin_top: 1,
            ) {
                View(
                    width: sidebar_width,
                    flex_direction: FlexDirection::Column,
                    border_style: BorderStyle::Round,
                    border_edges: Edges::Right,
                    border_color: Color::DarkGrey,
                    padding_right: 1,
                    gap: 0,
                ) {
                    View(
                        width: 100pct,
                        padding_left: 1,
                        padding_bottom: 1,
                    ) {
                        Text(
                            content: "SECTIONS",
                            color: Color::Grey,
                            weight: Weight::Bold,
                        )
                    }
                    #(categories)
                }

                View(
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    padding_left: 2,
                ) {
                    View(
                        width: 100pct,
                        height: 1,
                        align_items: AlignItems::Center,
                        gap: 1,
                    ) {
                        Text(
                            content: editor.category.label().to_string(),
                            color: theme.primary,
                            weight: Weight::Bold,
                        )
                        View(
                            flex_grow: 1.0,
                            height: 1,
                            border_style: BorderStyle::Round,
                            border_edges: Edges::Bottom,
                            border_color: Color::DarkGrey,
                        )
                    }
                    Text(
                        content: editor.category.description().to_string(),
                        color: Color::Grey,
                        wrap: TextWrap::NoWrap,
                    )
                    View(height: 1)
                    View(flex_grow: 1.0, width: 100pct) {
                        ScrollView(handle: handle, keyboard_scroll: false, scrollbar: false) {
                            View(flex_direction: FlexDirection::Column, width: 100pct) {
                                #(rows)
                            }
                        }
                    }
                    View(
                        width: 100pct,
                        height: 2,
                        border_style: BorderStyle::Round,
                        border_edges: Edges::Top,
                        border_color: Color::DarkGrey,
                        padding_left: 1,
                        overflow: Overflow::Hidden,
                    ) {
                        Text(
                            content: hint,
                            color: hint_color,
                            wrap: TextWrap::NoWrap,
                        )
                    }
                }
            }
        }
    }
    .into_any()
}

pub fn editor_overlay(theme: Theme, editor: &EditorState) -> Option<AnyElement<'static>> {
    if let Some(select) = &editor.select {
        let options = select_options(editor, select.field);
        let rows: Vec<AnyElement<'static>> = options
            .iter()
            .enumerate()
            .map(|(index, option)| {
                let active = index == select.selected;
                element! {
                    View(
                        width: 100pct,
                        height: 1,
                        padding_left: 1,
                        padding_right: 1,
                        background_color: if active { Some(Color::DarkGrey) } else { None },
                    ) {
                        View(width: 2) {
                            Text(
                                content: if active { "▌" } else { " " },
                                color: theme.primary,
                                weight: Weight::Bold,
                            )
                        }
                        View(flex_grow: 1.0) {
                            Text(
                                content: option.clone(),
                                color: if active { Color::White } else { Color::Grey },
                                weight: if active { Weight::Bold } else { Weight::Normal },
                                wrap: TextWrap::NoWrap,
                            )
                        }
                        View(width: 2) {
                            Text(
                                content: if active { "✓" } else { "" },
                                color: theme.primary,
                                weight: Weight::Bold,
                            )
                        }
                    }
                }
                .into_any()
            })
            .collect();

        return Some(
            element! {
                View(
                    position: Position::Absolute,
                    top: 0,
                    left: 0,
                    width: 100pct,
                    height: 100pct,
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                ) {
                    View(
                        width: 52,
                        max_width: 80pct,
                        flex_direction: FlexDirection::Column,
                        border_style: BorderStyle::Round,
                        border_color: theme.primary,
                        background_color: theme.background,
                        padding_left: 2,
                        padding_right: 2,
                        padding_top: 1,
                        padding_bottom: 1,
                        gap: 1,
                    ) {
                        View(width: 100pct, justify_content: JustifyContent::Center) {
                            Text(
                                content: format!("SELECT {}", select.field.label().to_uppercase()),
                                color: theme.primary,
                                weight: Weight::Bold,
                            )
                        }
                        #(rows)
                        View(width: 100pct, justify_content: JustifyContent::Center) {
                            Text(content: "Enter select · Esc cancel", color: Color::Grey)
                        }
                    }
                }
            }
            .into_any(),
        );
    }

    if editor.confirm {
        let profile = truncate(&editor.draft.id, 32);
        return Some(
            element! {
                View(
                    position: Position::Absolute,
                    top: 0,
                    left: 0,
                    width: 100pct,
                    height: 100pct,
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                ) {
                    View(
                        width: 56,
                        max_width: 80pct,
                        flex_direction: FlexDirection::Column,
                        border_style: BorderStyle::Round,
                        border_color: theme.secondary,
                        background_color: theme.background,
                        padding_left: 2,
                        padding_right: 2,
                        padding_top: 1,
                        padding_bottom: 1,
                        gap: 1,
                    ) {
                        View(width: 100pct, justify_content: JustifyContent::Center) {
                            Text(
                                content: "󰀪 UNSAVED CHANGES",
                                color: theme.secondary,
                                weight: Weight::Bold,
                            )
                        }
                        View(width: 100pct, justify_content: JustifyContent::Center) {
                            Text(
                                content: format!("Save changes to {profile}?"),
                                color: Color::White,
                                wrap: TextWrap::NoWrap,
                            )
                        }
                        View(width: 100pct, justify_content: JustifyContent::Center) {
                            Text(
                                content: "s Save · d Discard · Esc Cancel",
                                color: Color::Grey,
                            )
                        }
                    }
                }
            }
            .into_any(),
        );
    }

    None
}

pub fn footer_hint(width: u32) -> String {
    if width < 120 {
        "j/k Field · h/l Section · Enter Edit · Esc Back".to_string()
    } else {
        "↑/↓ Field  ·  ←/→ Section  ·  Enter Edit/Select  ·  Esc Save/Back".to_string()
    }
}
