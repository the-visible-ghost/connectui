use crate::network::{
    ConnectionType, DeviceInfo, DeviceKind, IpAddress, IpMethod, ProfileDraft, Security, WifiMode,
    parse_addresses, parse_ip_list, parse_search_list,
};
use crate::ui::common::Theme;
use crate::ui::notification::{Notification, NotificationLevel, notify};
use iocraft::components::{ScrollView, ScrollViewHandle};
use iocraft::hooks::Ref;
use iocraft::prelude::*;
use smol::channel::Sender;

use super::Command;

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
    ShowPassword,
    Bssid,
    WifiClonedMac,
    WifiMtu,
    Hidden,
    EthernetMac,
    EthernetClonedMac,
    EthernetMtu,
    Ipv4Method,
    Ipv4Address(usize),
    Ipv4AddAddress,
    Ipv4Gateway,
    Ipv4Dns,
    Ipv4Search,
    Ipv6Method,
    Ipv6Address(usize),
    Ipv6AddAddress,
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
            Self::ShowPassword => "Show Password",
            Self::Bssid => "BSSID",
            Self::WifiClonedMac => "Cloned MAC",
            Self::WifiMtu => "MTU",
            Self::Hidden => "Hidden",
            Self::EthernetMac => "MAC Address",
            Self::EthernetClonedMac => "Cloned MAC",
            Self::EthernetMtu => "MTU",
            Self::Ipv4Method => "Method",
            Self::Ipv4Address(_) => "Address",
            Self::Ipv4AddAddress => "Add Address",
            Self::Ipv4Gateway => "Gateway",
            Self::Ipv4Dns => "DNS Servers",
            Self::Ipv4Search => "Search Domains",
            Self::Ipv6Method => "Method",
            Self::Ipv6Address(_) => "Address",
            Self::Ipv6AddAddress => "Add Address",
            Self::Ipv6Gateway => "Gateway",
            Self::Ipv6Dns => "DNS Servers",
            Self::Ipv6Search => "Search Domains",
        }
    }

    fn display_label(self) -> String {
        match self {
            Self::Ipv4Address(index) | Self::Ipv6Address(index) => {
                format!("Address {}", index + 1)
            }
            _ => self.label().to_string(),
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
            Self::ShowPassword => "Reveal the password while you type it",
            Self::Bssid => "Optional access point MAC address",
            Self::WifiClonedMac => "Optional MAC address presented to the network",
            Self::WifiMtu => "Optional maximum transmission unit",
            Self::Hidden => "Connect to a network that does not broadcast its SSID",
            Self::EthernetMac => "Optional permanent MAC address",
            Self::EthernetClonedMac => "Optional MAC address presented to the network",
            Self::EthernetMtu => "Optional maximum transmission unit",
            Self::Ipv4Method | Self::Ipv6Method => "Select how addresses are assigned",
            Self::Ipv4Address(_) => "IPv4 address in address/prefix form",
            Self::Ipv4AddAddress => "Add another IPv4 address",
            Self::Ipv6Address(_) => "IPv6 address in address/prefix form",
            Self::Ipv6AddAddress => "Add another IPv6 address",
            Self::Ipv4Gateway | Self::Ipv6Gateway => "Optional default gateway",
            Self::Ipv4Dns | Self::Ipv6Dns => "Comma-separated DNS server addresses",
            Self::Ipv4Search | Self::Ipv6Search => "Comma-separated search domains",
        }
    }

    fn is_toggle(self) -> bool {
        matches!(
            self,
            Self::Autoconnect | Self::AllUsers | Self::Hidden | Self::ShowPassword
        )
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

    fn is_add_address(self) -> bool {
        matches!(self, Self::Ipv4AddAddress | Self::Ipv6AddAddress)
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
    pub show_password: bool,
}

#[derive(Clone)]
pub enum EditorAction {
    None,
    Close,
    Save(Box<ProfileDraft>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SectionRow {
    Space,
    Group(&'static str),
    Field(Field),
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
            show_password: false,
        }
    }

    pub fn dirty(&self) -> bool {
        self.is_new || self.draft != self.original
    }

    pub fn selected_span(&self) -> (usize, usize) {
        if let Some(select) = &self.select {
            let (top, _) = self.field_span(select.field);
            return (top, 1 + popup_height(self, select.field));
        }
        self.field_span(self.current_field())
    }

    fn field_span(&self, field: Field) -> (usize, usize) {
        self.section_rows(self.category)
            .iter()
            .position(|row| matches!(row, SectionRow::Field(candidate) if *candidate == field))
            .map(|index| (index, 1))
            .unwrap_or((0, 1))
    }

    fn section_fields(&self, category: EditorCategory) -> Vec<Field> {
        self.section_rows(category)
            .into_iter()
            .filter_map(|row| match row {
                SectionRow::Field(field) => Some(field),
                SectionRow::Space | SectionRow::Group(_) => None,
            })
            .collect()
    }

    fn section_rows(&self, category: EditorCategory) -> Vec<SectionRow> {
        let mut rows = vec![SectionRow::Space];
        match category {
            EditorCategory::Network => {
                rows.extend([
                    SectionRow::Group("General"),
                    SectionRow::Field(Field::ConnectionName),
                    SectionRow::Field(Field::Device),
                    SectionRow::Field(Field::Autoconnect),
                    SectionRow::Field(Field::AllUsers),
                ]);
                match self.draft.connection_type {
                    ConnectionType::Wifi => {
                        rows.extend([
                            SectionRow::Space,
                            SectionRow::Group("Wireless"),
                            SectionRow::Field(Field::Ssid),
                            SectionRow::Field(Field::WifiMode),
                            SectionRow::Field(Field::Security),
                        ]);
                        if self
                            .draft
                            .wifi
                            .as_ref()
                            .is_some_and(|wifi| needs_password(wifi.security))
                        {
                            rows.push(SectionRow::Field(Field::Password));
                            rows.push(SectionRow::Field(Field::ShowPassword));
                        }
                        rows.extend([
                            SectionRow::Space,
                            SectionRow::Group("Advanced"),
                            SectionRow::Field(Field::Bssid),
                            SectionRow::Field(Field::WifiClonedMac),
                            SectionRow::Field(Field::WifiMtu),
                            SectionRow::Field(Field::Hidden),
                        ]);
                    }
                    ConnectionType::Ethernet => {
                        rows.extend([
                            SectionRow::Space,
                            SectionRow::Group("Hardware"),
                            SectionRow::Field(Field::EthernetMac),
                            SectionRow::Field(Field::EthernetClonedMac),
                            SectionRow::Field(Field::EthernetMtu),
                        ]);
                    }
                }
            }
            EditorCategory::Ipv4 => {
                rows.extend([
                    SectionRow::Group("Configuration"),
                    SectionRow::Field(Field::Ipv4Method),
                ]);
                if self.draft.ipv4.method == IpMethod::Manual {
                    rows.extend([SectionRow::Space, SectionRow::Group("Addresses")]);
                    rows.extend(
                        (0..self.draft.ipv4.addresses.len())
                            .map(|index| SectionRow::Field(Field::Ipv4Address(index))),
                    );
                    rows.push(SectionRow::Field(Field::Ipv4AddAddress));
                    rows.extend([
                        SectionRow::Space,
                        SectionRow::Group("Routing"),
                        SectionRow::Field(Field::Ipv4Gateway),
                    ]);
                }
                if self.draft.ipv4.method != IpMethod::Disabled {
                    rows.extend([
                        SectionRow::Space,
                        SectionRow::Group("DNS"),
                        SectionRow::Field(Field::Ipv4Dns),
                        SectionRow::Field(Field::Ipv4Search),
                    ]);
                }
            }
            EditorCategory::Ipv6 => {
                rows.extend([
                    SectionRow::Group("Configuration"),
                    SectionRow::Field(Field::Ipv6Method),
                ]);
                if self.draft.ipv6.method == IpMethod::Manual {
                    rows.extend([SectionRow::Space, SectionRow::Group("Addresses")]);
                    rows.extend(
                        (0..self.draft.ipv6.addresses.len())
                            .map(|index| SectionRow::Field(Field::Ipv6Address(index))),
                    );
                    rows.push(SectionRow::Field(Field::Ipv6AddAddress));
                    rows.extend([
                        SectionRow::Space,
                        SectionRow::Group("Routing"),
                        SectionRow::Field(Field::Ipv6Gateway),
                    ]);
                }
                if self.draft.ipv6.method != IpMethod::Disabled {
                    rows.extend([
                        SectionRow::Space,
                        SectionRow::Group("DNS"),
                        SectionRow::Field(Field::Ipv6Dns),
                        SectionRow::Field(Field::Ipv6Search),
                    ]);
                }
            }
        }
        rows
    }

    fn all_fields(&self) -> Vec<(EditorCategory, Field)> {
        EditorCategory::ALL
            .into_iter()
            .flat_map(|category| {
                self.section_fields(category)
                    .into_iter()
                    .map(move |field| (category, field))
            })
            .collect()
    }

    fn current_field(&self) -> Field {
        let fields = self.all_fields();
        fields[self.focus.min(fields.len().saturating_sub(1))].1
    }

    fn clamp_focus(&mut self) {
        let fields = self.all_fields();
        if fields.is_empty() {
            self.focus = 0;
        } else {
            self.focus = self.focus.min(fields.len() - 1);
            self.category = fields[self.focus].0;
        }
    }

    fn focus_field(&mut self, field: Field) {
        let fields = self.all_fields();
        if let Some(index) = fields.iter().position(|(_, candidate)| *candidate == field) {
            self.focus = index;
            self.category = fields[index].0;
        }
    }

    fn move_focus(&mut self, delta: isize) {
        let fields = self.section_fields(self.category);
        if fields.is_empty() {
            return;
        }
        let current = fields
            .iter()
            .position(|field| *field == self.current_field())
            .unwrap_or(0);
        let next = (current as isize + delta).rem_euclid(fields.len() as isize) as usize;
        self.focus_field(fields[next]);
    }

    fn move_section(&mut self, delta: isize) {
        let target = self.category.next(delta);
        if let Some(index) = self.first_field_index(target) {
            self.category = target;
            self.focus = index;
        }
    }

    fn first_field_index(&self, category: EditorCategory) -> Option<usize> {
        self.all_fields()
            .iter()
            .position(|(candidate, _)| *candidate == category)
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
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Left | KeyCode::Char('h') => {
            editor.move_focus(-1)
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Right | KeyCode::Char('l') => {
            editor.move_focus(1)
        }
        KeyCode::BackTab => editor.move_section(-1),
        KeyCode::Tab => editor.move_section(1),
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
        KeyCode::Tab => {
            editor.select = None;
            apply_select(editor, field, selected);
            editor.move_section(1);
        }
        KeyCode::BackTab => {
            editor.select = None;
            apply_select(editor, field, selected);
            editor.move_section(-1);
        }
        _ => {}
    }
}

fn handle_input_key(editor: &mut EditorState, code: KeyCode) {
    if code == KeyCode::Esc {
        cancel_input(editor);
        return;
    }
    if code == KeyCode::Enter {
        commit_pending_input(editor);
        editor.clamp_focus();
        return;
    }
    if code == KeyCode::Tab {
        if commit_pending_input(editor) {
            editor.move_section(1);
        }
        return;
    }
    if code == KeyCode::BackTab {
        if commit_pending_input(editor) {
            editor.move_section(-1);
        }
        return;
    }

    let Some(input) = editor.input.as_mut() else {
        return;
    };
    match code {
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

fn commit_pending_input(editor: &mut EditorState) -> bool {
    let Some(input) = editor.input.take() else {
        return true;
    };
    match commit_input(editor, input.field, &input.value) {
        Ok(()) => {
            editor.error = None;
            true
        }
        Err(message) => {
            editor.error = Some(message);
            let cursor = input.value.len();
            editor.input = Some(InputState {
                field: input.field,
                value: input.value,
                cursor,
            });
            false
        }
    }
}

fn cancel_input(editor: &mut EditorState) {
    if let Some(input) = editor.input.take() {
        match input.field {
            Field::Ipv4Address(index) => {
                remove_empty_address(&mut editor.draft.ipv4.addresses, index)
            }
            Field::Ipv6Address(index) => {
                remove_empty_address(&mut editor.draft.ipv6.addresses, index)
            }
            _ => {}
        }
    }
    editor.clamp_focus();
    editor.error = None;
}

fn remove_empty_address(addresses: &mut Vec<IpAddress>, index: usize) {
    if addresses
        .get(index)
        .is_some_and(|address| address.address.is_empty())
    {
        addresses.remove(index);
    }
}

fn add_address(editor: &mut EditorState, field: Field) {
    match field {
        Field::Ipv4AddAddress => {
            let index = editor.draft.ipv4.addresses.len();
            editor.draft.ipv4.addresses.push(IpAddress {
                address: String::new(),
                prefix: 0,
            });
            editor.focus_field(Field::Ipv4Address(index));
            editor.input = Some(InputState {
                field: Field::Ipv4Address(index),
                value: String::new(),
                cursor: 0,
            });
        }
        Field::Ipv6AddAddress => {
            let index = editor.draft.ipv6.addresses.len();
            editor.draft.ipv6.addresses.push(IpAddress {
                address: String::new(),
                prefix: 0,
            });
            editor.focus_field(Field::Ipv6Address(index));
            editor.input = Some(InputState {
                field: Field::Ipv6Address(index),
                value: String::new(),
                cursor: 0,
            });
        }
        _ => {}
    }
    editor.error = None;
}

fn activate_field(editor: &mut EditorState, field: Field) {
    if field.is_add_address() {
        add_address(editor, field);
        return;
    }
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
        Field::ShowPassword => editor.show_password = !editor.show_password,
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
        Field::Ipv4Address(index) => editor
            .draft
            .ipv4
            .addresses
            .get(index)
            .map(IpAddress::display)
            .unwrap_or_default(),
        Field::Ipv4Gateway => editor.draft.ipv4.gateway.clone(),
        Field::Ipv4Dns => editor.draft.ipv4.dns_text(),
        Field::Ipv4Search => editor.draft.ipv4.search_text(),
        Field::Ipv6Address(index) => editor
            .draft
            .ipv6
            .addresses
            .get(index)
            .map(IpAddress::display)
            .unwrap_or_default(),
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
        Field::Ipv4Address(index) => {
            commit_address(&mut editor.draft.ipv4.addresses, index, value, false)?;
        }
        Field::Ipv4Gateway => editor.draft.ipv4.gateway = value.trim().to_string(),
        Field::Ipv4Dns => editor.draft.ipv4.dns = parse_ip_list(value, false)?,
        Field::Ipv4Search => editor.draft.ipv4.search = parse_search_list(value),
        Field::Ipv6Address(index) => {
            commit_address(&mut editor.draft.ipv6.addresses, index, value, true)?;
        }
        Field::Ipv6Gateway => editor.draft.ipv6.gateway = value.trim().to_string(),
        Field::Ipv6Dns => editor.draft.ipv6.dns = parse_ip_list(value, true)?,
        Field::Ipv6Search => editor.draft.ipv6.search = parse_search_list(value),
        _ => {}
    }
    Ok(())
}

fn commit_address(
    addresses: &mut Vec<IpAddress>,
    index: usize,
    value: &str,
    ipv6: bool,
) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        if index < addresses.len() {
            addresses.remove(index);
        }
        return Ok(());
    }
    let mut parsed = parse_addresses(value, ipv6)?;
    if parsed.len() != 1 {
        return Err("Enter one address per row".to_string());
    }
    let address = parsed.remove(0);
    if index < addresses.len() {
        addresses[index] = address;
    } else {
        addresses.push(address);
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

fn popup_height(editor: &EditorState, field: Field) -> usize {
    select_options(editor, field).len() + 2
}

fn popup_width(editor: &EditorState, field: Field, available: usize) -> u32 {
    let longest = select_options(editor, field)
        .iter()
        .map(|option| option.chars().count())
        .max()
        .unwrap_or(8);
    let width = (longest + 4).clamp(18, 32);
    width.min(available.max(1)) as u32
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
        if field.is_password() && !editor.show_password {
            return format!(
                "{}{}",
                "*".repeat(input.value.chars().count()),
                cursor_mark()
            );
        }
        let mut value = input.value.clone();
        value.insert_str(input.cursor, cursor_mark());
        return value;
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
        Field::ShowPassword => yes_no(editor.show_password),
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
        Field::Ipv4Address(index) => editor
            .draft
            .ipv4
            .addresses
            .get(index)
            .map(IpAddress::display)
            .unwrap_or_else(|| "Not set".to_string()),
        Field::Ipv4AddAddress => String::new(),
        Field::Ipv4Gateway => optional_value(&editor.draft.ipv4.gateway, "None"),
        Field::Ipv4Dns => optional_value(&editor.draft.ipv4.dns_text(), "Automatic"),
        Field::Ipv4Search => optional_value(&editor.draft.ipv4.search_text(), "None"),
        Field::Ipv6Method => editor.draft.ipv6.method.label().to_string(),
        Field::Ipv6Address(index) => editor
            .draft
            .ipv6
            .addresses
            .get(index)
            .map(IpAddress::display)
            .unwrap_or_else(|| "Not set".to_string()),
        Field::Ipv6AddAddress => String::new(),
        Field::Ipv6Gateway => optional_value(&editor.draft.ipv6.gateway, "None"),
        Field::Ipv6Dns => optional_value(&editor.draft.ipv6.dns_text(), "Automatic"),
        Field::Ipv6Search => optional_value(&editor.draft.ipv6.search_text(), "None"),
    }
}

fn display_value_windowed(editor: &EditorState, field: Field, width: usize) -> String {
    if let Some(input) = editor.input.as_ref().filter(|input| input.field == field) {
        let cursor = input.value[..input.cursor].chars().count();
        let mut characters = if field.is_password() && !editor.show_password {
            vec!['*'; input.value.chars().count()]
        } else {
            input.value.chars().collect::<Vec<_>>()
        };
        characters.insert(cursor, '▏');
        return window_characters(&characters, cursor, width);
    }
    truncate(&display_value(editor, field), width)
}

fn window_characters(characters: &[char], cursor: usize, width: usize) -> String {
    let width = width.max(3);
    if characters.len() <= width {
        return characters.iter().collect();
    }
    let cursor = cursor.min(characters.len() - 1);
    let mut start = cursor.saturating_sub(width / 2);
    let mut end = (start + width).min(characters.len());
    if end == characters.len() {
        start = end.saturating_sub(width);
    }
    let left = start > 0;
    let right = end < characters.len();
    let reserve = left as usize + right as usize;
    if reserve > 0 && width > reserve {
        let content_width = width - reserve;
        start = cursor.saturating_sub(content_width / 2);
        end = (start + content_width).min(characters.len());
        start = end.saturating_sub(content_width);
    }
    let left = start > 0;
    let right = end < characters.len();
    let mut value = String::new();
    if left {
        value.push('…');
    }
    value.extend(&characters[start..end]);
    if right {
        value.push('…');
    }
    value
}

fn toggle_value(editor: &EditorState, field: Field) -> bool {
    match field {
        Field::Autoconnect => editor.draft.autoconnect,
        Field::AllUsers => editor.draft.all_users,
        Field::Hidden => editor.draft.wifi.as_ref().is_some_and(|wifi| wifi.hidden),
        Field::ShowPassword => editor.show_password,
        _ => false,
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
        Field::Autoconnect | Field::AllUsers | Field::Hidden | Field::ShowPassword => {
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
        Field::Ipv4Address(_) | Field::Ipv6Address(_) => {
            if value == "Not set" {
                Color::Grey
            } else {
                Color::White
            }
        }
        Field::Ipv4AddAddress | Field::Ipv6AddAddress => theme.primary,
        _ => {
            if value == "None" || value == "Automatic" || value == "Permanent" {
                Color::Grey
            } else {
                Color::White
            }
        }
    }
}

fn field_suffix(editor: &EditorState, field: Field) -> &'static str {
    if field.is_add_address() {
        "+"
    } else if field.is_select() {
        if editor
            .select
            .as_ref()
            .is_some_and(|select| select.field == field)
        {
            "󰅀"
        } else {
            "󰅂"
        }
    } else if field.is_toggle() {
        ""
    } else {
        "✎"
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

#[derive(Clone)]
pub struct EditorUi {
    state: State<Option<EditorState>>,
    modal_active: State<bool>,
    footer: Option<State<Option<String>>>,
    notifications: State<Vec<Notification>>,
    tx: Sender<Command>,
}

impl EditorUi {
    pub fn new(
        state: State<Option<EditorState>>,
        modal_active: State<bool>,
        footer: Option<State<Option<String>>>,
        notifications: State<Vec<Notification>>,
        tx: Sender<Command>,
    ) -> Self {
        Self {
            state,
            modal_active,
            footer,
            notifications,
            tx,
        }
    }

    fn close_editor(&mut self, discarded: bool) {
        self.state.set(None);
        self.modal_active.set(false);
        if let Some(mut footer) = self.footer {
            footer.set(None);
        }
        if discarded {
            notify(
                self.notifications,
                NotificationLevel::Info,
                "Profile editor",
                "Changes discarded",
            );
        }
    }

    fn cancel_confirm(&mut self) {
        let mut current = (*self.state.read()).clone();
        if let Some(editor) = current.as_mut() {
            editor.confirm = false;
        }
        self.state.set(current);
    }

    pub(crate) fn request_save(&mut self) {
        self.save(false);
    }

    fn confirm_save(&mut self) {
        self.save(true);
    }

    fn save(&mut self, confirmed: bool) {
        let mut current = (*self.state.read()).clone();
        let Some(editor) = current.as_mut() else {
            return;
        };
        if editor.saving || (editor.confirm && !confirmed) {
            return;
        }
        editor.confirm = false;
        if let Some(select) = editor.select.take() {
            apply_select(editor, select.field, select.selected);
        }
        if !commit_pending_input(editor) {
            self.state.set(current);
            return;
        }
        if !editor.dirty() {
            self.state.set(current);
            return;
        }
        editor.saving = true;
        let draft = Box::new(editor.draft.clone());
        self.state.set(current);
        if self.tx.try_send(Command::SaveProfile { draft }).is_err() {
            let mut current = (*self.state.read()).clone();
            if let Some(editor) = current.as_mut() {
                editor.saving = false;
                editor.error = Some("Could not save the profile".to_string());
            }
            self.state.set(current);
        }
    }

    fn request_close(&mut self) {
        let mut current = (*self.state.read()).clone();
        let Some(editor) = current.as_mut() else {
            return;
        };
        if editor.saving || editor.confirm {
            return;
        }
        editor.select = None;
        if !commit_pending_input(editor) {
            self.state.set(current);
            return;
        }
        if editor.dirty() {
            editor.confirm = true;
            self.state.set(current);
        } else {
            self.close_editor(false);
        }
    }

    fn click_field(&mut self, field: Field) {
        let mut current = (*self.state.read()).clone();
        let Some(editor) = current.as_mut() else {
            return;
        };
        if editor.saving || editor.confirm {
            return;
        }
        if field == Field::ShowPassword
            && editor
                .input
                .as_ref()
                .is_some_and(|input| input.field == Field::Password)
        {
            editor.show_password = !editor.show_password;
            self.state.set(current);
            return;
        }
        let different_input = editor
            .input
            .as_ref()
            .is_some_and(|input| input.field != field);
        if different_input && !commit_pending_input(editor) {
            self.state.set(current);
            return;
        }
        if editor
            .select
            .as_ref()
            .is_some_and(|select| select.field != field)
        {
            editor.select = None;
        }
        editor.focus_field(field);
        if editor
            .input
            .as_ref()
            .is_some_and(|input| input.field == field)
        {
            self.state.set(current);
            return;
        }
        if editor
            .select
            .as_ref()
            .is_some_and(|select| select.field == field)
        {
            editor.select = None;
            self.state.set(current);
            return;
        }
        activate_field(editor, field);
        self.state.set(current);
    }

    fn cancel_select(&mut self) {
        let mut current = (*self.state.read()).clone();
        let Some(editor) = current.as_mut() else {
            return;
        };
        if editor.saving || editor.confirm || editor.select.is_none() {
            return;
        }
        editor.select = None;
        self.state.set(current);
    }

    fn click_section(&mut self, category: EditorCategory) {
        let mut current = (*self.state.read()).clone();
        let Some(editor) = current.as_mut() else {
            return;
        };
        if editor.saving || editor.confirm {
            return;
        }
        if !commit_pending_input(editor) {
            self.state.set(current);
            return;
        }
        editor.select = None;
        if let Some(index) = editor.first_field_index(category) {
            editor.category = category;
            editor.focus = index;
        }
        self.state.set(current);
    }

    fn click_option(&mut self, field: Field, index: usize) {
        let mut current = (*self.state.read()).clone();
        let Some(editor) = current.as_mut() else {
            return;
        };
        if editor.saving || editor.confirm {
            return;
        }
        editor.select = None;
        editor.focus_field(field);
        apply_select(editor, field, index);
        self.state.set(current);
    }
}

fn group_heading(label: &str) -> AnyElement<'static> {
    element! {
        View(
            width: 100pct,
            height: 1,
            padding_left: 1,
            padding_right: 1,
            align_items: AlignItems::Center,
        ) {
            Text(
                content: label.to_uppercase(),
                color: Color::Grey,
                weight: Weight::Bold,
                wrap: TextWrap::NoWrap,
            )
            View(
                flex_grow: 1.0,
                height: 1,
                margin_left: 1,
                border_style: BorderStyle::Round,
                border_edges: Edges::Bottom,
                border_color: Color::DarkGrey,
            )
        }
    }
    .into_any()
}

fn section_tab(
    theme: Theme,
    editor: &EditorState,
    category: EditorCategory,
    ui: EditorUi,
) -> AnyElement<'static> {
    let active = category == editor.category;
    let interactive = editor.select.is_none();
    let color = if active { theme.primary } else { Color::Grey };
    let tab = element! {
        View(
            height: 1,
            padding_left: 1,
            padding_right: 1,
            background_color: if active { Some(Color::DarkGrey) } else { None },
            align_items: AlignItems::Center,
        ) {
            View(width: 2) {
                Text(
                    content: category.icon().to_string(),
                    color,
                    weight: Weight::Bold,
                )
            }
            Text(
                content: category.label(),
                color,
                weight: if active { Weight::Bold } else { Weight::Normal },
                wrap: TextWrap::NoWrap,
            )
        }
    }
    .into_any();
    if interactive {
        let mut tab_ui = ui;
        element! {
            Button(handler: move |_| tab_ui.click_section(category)) {
                #(Some(tab))
            }
        }
        .into_any()
    } else {
        tab
    }
}

fn field_label_width(width: u32) -> u32 {
    if width >= 100 { 24 } else { 18 }
}

fn field_row(
    theme: Theme,
    editor: &EditorState,
    field: Field,
    selected: bool,
    width: u32,
    ui: EditorUi,
) -> AnyElement<'static> {
    let label_width = field_label_width(width);
    let value_width = width.saturating_sub(15 + label_width) as usize;
    let value = display_value_windowed(editor, field, value_width);
    let color = field_color(theme, editor, field);
    let editing = editor
        .input
        .as_ref()
        .is_some_and(|input| input.field == field);
    let value_text = if field.is_toggle() {
        format!(
            "{} {}",
            if toggle_value(editor, field) {
                "[x]"
            } else {
                "[ ]"
            },
            value
        )
    } else {
        value
    };
    let value_color = if editing { theme.primary } else { color };
    let suffix = field_suffix(editor, field);
    let background = if selected {
        Some(Color::DarkGrey)
    } else {
        None
    };
    let label_color = if selected { theme.primary } else { Color::Grey };
    let label_weight = if selected {
        Weight::Bold
    } else {
        Weight::Normal
    };
    let value_weight = if selected {
        Weight::Bold
    } else {
        Weight::Normal
    };
    let select_field = editor.select.as_ref().map(|select| select.field);
    let label_interactive = select_field.is_none() || select_field == Some(field);
    let value_interactive = select_field.is_none();
    let label_view = element! {
        View(
            width: label_width,
            height: 1,
            flex_shrink: 0.0,
            overflow: Overflow::Hidden,
            align_items: AlignItems::Center,
        ) {
            Text(
                content: field.display_label(),
                color: label_color,
                weight: label_weight,
                wrap: TextWrap::NoWrap,
            )
        }
    }
    .into_any();
    let value_view = element! {
        View(
            width: 100pct,
            height: 1,
            overflow: Overflow::Hidden,
            align_items: AlignItems::Center,
        ) {
            Text(
                content: value_text,
                color: value_color,
                weight: value_weight,
                wrap: TextWrap::NoWrap,
            )
        }
    }
    .into_any();
    let label = if label_interactive {
        let mut label_ui = ui.clone();
        element! {
            Button(handler: move |_| label_ui.click_field(field)) {
                #(Some(label_view))
            }
        }
        .into_any()
    } else {
        label_view
    };
    let value_cell = if value_interactive {
        let mut value_ui = ui;
        element! {
            Button(handler: move |_| value_ui.click_field(field)) {
                #(Some(value_view))
            }
        }
        .into_any()
    } else {
        value_view
    };

    element! {
        View(
            width: 100pct,
            height: 1,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding_left: 1,
            padding_right: 1,
            background_color: background,
        ) {
            View(width: 1, flex_shrink: 0.0) {
                Text(
                    content: if selected { "▌" } else { " " },
                    color: theme.primary,
                    weight: Weight::Bold,
                )
            }
            View(width: 1, flex_shrink: 0.0)
            #(Some(label))
            View(width: 1, flex_shrink: 0.0)
            View(flex_grow: 1.0, flex_basis: FlexBasis::Length(0), min_width: 0, height: 1) {
                #(Some(value_cell))
            }
            View(
                width: 2,
                height: 1,
                flex_shrink: 0.0,
                justify_content: JustifyContent::FlexEnd,
            ) {
                Text(
                    content: suffix.to_string(),
                    color: if selected { theme.primary } else { Color::DarkGrey },
                )
            }
        }
    }
    .into_any()
}

fn select_popup(
    theme: Theme,
    editor: &EditorState,
    field: Field,
    width: u32,
    top: usize,
    left: u32,
    ui: EditorUi,
) -> AnyElement<'static> {
    let options = select_options(editor, field);
    let selected = editor
        .select
        .as_ref()
        .map(|select| select.selected)
        .unwrap_or(0);
    let popup_width = popup_width(editor, field, width as usize);
    let text_width = popup_width.saturating_sub(4) as usize;
    let option_elements: Vec<AnyElement<'static>> = options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            let active = index == selected;
            let mut option_ui = ui.clone();
            element! {
                Button(handler: move |_| option_ui.click_option(field, index)) {
                    View(
                        width: 100pct,
                        height: 1,
                        padding_left: 1,
                        padding_right: 1,
                        background_color: if active { Color::DarkGrey } else { Color::Black },
                    ) {
                        Text(
                            content: truncate(
                                &format!("{} {}", if active { "✓" } else { " " }, option),
                                text_width,
                            ),
                            color: if active { Color::White } else { Color::Grey },
                            weight: if active { Weight::Bold } else { Weight::Normal },
                            wrap: TextWrap::NoWrap,
                        )
                    }
                }
            }
            .into_any()
        })
        .collect();

    element! {
        View(
            position: Position::Absolute,
            top: top as i32,
            left: left as i32,
            width: popup_width,
            flex_direction: FlexDirection::Column,
            border_style: BorderStyle::Round,
            border_color: theme.primary,
            background_color: Color::Black,
        ) {
            #(option_elements)
        }
    }
    .into_any()
}

pub fn editor_panel(
    theme: Theme,
    editor: &EditorState,
    width: u32,
    handle: Ref<ScrollViewHandle>,
    ui: EditorUi,
) -> AnyElement<'static> {
    let tabs: Vec<AnyElement<'static>> = EditorCategory::ALL
        .into_iter()
        .map(|category| section_tab(theme, editor, category, ui.clone()))
        .collect();
    let rows: Vec<AnyElement<'static>> = editor
        .section_rows(editor.category)
        .into_iter()
        .map(|row| match row {
            SectionRow::Space => element! {
                View(width: 100pct, height: 1)
            }
            .into_any(),
            SectionRow::Group(label) => group_heading(label),
            SectionRow::Field(field) => field_row(
                theme,
                editor,
                field,
                editor.current_field() == field,
                width,
                ui.clone(),
            ),
        })
        .collect();
    let popup = editor.select.as_ref().map(|select| {
        let (top, _) = editor.field_span(select.field);
        let left = 4 + field_label_width(width);
        let available = width.saturating_sub(15 + field_label_width(width));
        select_popup(
            theme,
            editor,
            select.field,
            available,
            top + 1,
            left,
            ui.clone(),
        )
    });
    let popup_spacer = editor.select.as_ref().map(|select| {
        let height = popup_height(editor, select.field) as u32;
        element! {
            View(width: 100pct, height: height)
        }
        .into_any()
    });
    let shield = editor.select.as_ref().map(|_| {
        let mut shield_ui = ui.clone();
        element! {
            View(
                position: Position::Absolute,
                top: 0,
                left: 0,
                width: 100pct,
                height: 100pct,
            ) {
                Button(handler: move |_| shield_ui.cancel_select()) {
                    View(width: 100pct, height: 100pct)
                }
            }
        }
        .into_any()
    });
    let profile_kind = editor.draft.connection_type.label();
    let profile_id = truncate(&editor.draft.id, width.saturating_sub(40) as usize);
    let hint = editor
        .error
        .clone()
        .unwrap_or_else(|| editor.current_field().hint().to_string());
    let hint_color = if editor.error.is_some() {
        Color::Red
    } else {
        Color::Grey
    };
    let dirty = editor.dirty();
    let can_save = dirty || editor.input.is_some() || editor.select.is_some();
    let status = if editor.saving {
        "Saving…"
    } else if editor.is_new {
        "Not saved yet"
    } else if dirty {
        "● Unsaved changes"
    } else {
        "No changes"
    };
    let status_color = if editor.saving || dirty {
        theme.secondary
    } else {
        Color::Grey
    };
    let badge_background = if editor.is_new {
        theme.secondary_dark
    } else {
        Color::DarkGrey
    };
    let badge_color = if editor.is_new {
        theme.secondary
    } else {
        Color::Grey
    };
    let save_color = if can_save {
        Color::DarkGreen
    } else {
        Color::DarkGrey
    };
    let save_text_color = if can_save { Color::White } else { Color::Grey };
    let section_description = (width >= 100).then(|| {
        element! {
            Text(
                content: editor.category.description(),
                color: Color::Grey,
                wrap: TextWrap::NoWrap,
            )
        }
        .into_any()
    });
    let mut save_ui = ui.clone();
    let mut close_ui = ui;

    element! {
        View(
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
        ) {
            View(
                width: 100pct,
                height: 1,
                padding_left: 1,
                padding_right: 1,
                align_items: AlignItems::Center,
                overflow: Overflow::Hidden,
            ) {
                Text(
                    content: "󰒓 Edit connection",
                    color: theme.primary,
                    weight: Weight::Bold,
                    wrap: TextWrap::NoWrap,
                )
                View(width: 2, flex_shrink: 0.0)
                Text(
                    content: profile_id,
                    color: Color::White,
                    wrap: TextWrap::NoWrap,
                )
                View(flex_grow: 1.0)
                View(
                    background_color: badge_background,
                    padding_left: 1,
                    padding_right: 1,
                ) {
                    Text(
                        content: if editor.is_new { "NEW" } else { "SAVED" },
                        color: badge_color,
                        weight: Weight::Bold,
                        wrap: TextWrap::NoWrap,
                    )
                }
            }

            View(
                width: 100pct,
                height: 1,
                padding_left: 1,
                padding_right: 1,
                align_items: AlignItems::Center,
                overflow: Overflow::Hidden,
            ) {
                Text(
                    content: format!(
                        "{profile_kind} · {}",
                        if editor.is_new { "New profile" } else { "Saved profile" }
                    ),
                    color: Color::Grey,
                    wrap: TextWrap::NoWrap,
                )
            }

            View(
                width: 100pct,
                height: 1,
                margin_top: 1,
                padding_left: 1,
                padding_right: 1,
                align_items: AlignItems::Center,
                gap: 1,
                overflow: Overflow::Hidden,
            ) {
                #(tabs)
                View(flex_grow: 1.0)
                #(section_description)
            }

            View(
                width: 100pct,
                height: 1,
                border_style: BorderStyle::Round,
                border_edges: Edges::Bottom,
                border_color: Color::DarkGrey,
            )

            View(
                width: 100pct,
                flex_grow: 1.0,
            ) {
                ScrollView(
                    handle: handle,
                    keyboard_scroll: false,
                    scrollbar: true,
                    scrollbar_thumb_color: theme.primary,
                    scrollbar_track_color: Color::DarkGrey,
                ) {
                    View(
                        flex_direction: FlexDirection::Column,
                        width: 100pct,
                        padding_right: 1,
                    ) {
                        #(rows)
                        #(popup_spacer)
                        #(popup)
                    }
                }
                #(shield)
            }

            View(
                width: 100pct,
                height: 1,
                margin_top: 1,
                padding_left: 1,
                padding_right: 1,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                gap: 1,
            ) {
                Button(handler: move |_| save_ui.request_save()) {
                    View(
                        background_color: save_color,
                        padding_left: 1,
                        padding_right: 1,
                    ) {
                        Text(
                            content: "Save",
                            color: save_text_color,
                            weight: Weight::Bold,
                        )
                    }
                }
                Button(handler: move |_| close_ui.request_close()) {
                    View(
                        background_color: Color::DarkGrey,
                        padding_left: 1,
                        padding_right: 1,
                    ) {
                        Text(content: "Close", color: Color::White)
                    }
                }
                View(flex_grow: 1.0)
                Text(
                    content: status,
                    color: status_color,
                    weight: if editor.saving || dirty {
                        Weight::Bold
                    } else {
                        Weight::Normal
                    },
                    wrap: TextWrap::NoWrap,
                )
            }

            View(
                width: 100pct,
                height: 1,
                margin_top: 1,
                padding_left: 1,
                padding_right: 1,
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
    .into_any()
}

pub fn editor_overlay(
    theme: Theme,
    editor: &EditorState,
    ui: EditorUi,
) -> Option<AnyElement<'static>> {
    if !editor.confirm {
        return None;
    }

    let profile = truncate(&editor.draft.id, 32);
    let mut save_ui = ui.clone();
    let mut discard_ui = ui.clone();
    let mut cancel_ui = ui;

    Some(
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
                    background_color: Color::Black,
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
                    View(
                        width: 100pct,
                        justify_content: JustifyContent::Center,
                        gap: 1,
                    ) {
                        Button(handler: move |_| save_ui.confirm_save()) {
                            View(
                                background_color: Color::DarkGreen,
                                padding_left: 1,
                                padding_right: 1,
                            ) {
                                Text(
                                    content: "s Save",
                                    color: Color::White,
                                    weight: Weight::Bold,
                                )
                            }
                        }
                        Button(handler: move |_| discard_ui.close_editor(true)) {
                            View(
                                background_color: Color::DarkRed,
                                padding_left: 1,
                                padding_right: 1,
                            ) {
                                Text(content: "d Discard", color: Color::White)
                            }
                        }
                        Button(handler: move |_| cancel_ui.cancel_confirm()) {
                            View(
                                background_color: Color::DarkGrey,
                                padding_left: 1,
                                padding_right: 1,
                            ) {
                                Text(content: "Esc Cancel", color: Color::White)
                            }
                        }
                    }
                }
            }
        }
        .into_any(),
    )
}

pub fn footer_hint(width: u32) -> String {
    if width < 100 {
        "j/k Field · Tab Section · Enter Edit · Ctrl+S Save · Esc Back".to_string()
    } else {
        "j/k/Arrows Field  ·  Tab/Shift+Tab Section  ·  Enter Edit  ·  Ctrl+S Save  ·  Esc Back"
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor_state() -> EditorState {
        EditorState::new(
            ProfileDraft::new_wifi("Test", Security::Wpa2, "wlan0"),
            Vec::new(),
            true,
        )
    }

    #[test]
    fn focus_wraps_within_active_section() {
        let mut editor = editor_state();
        let network_count = editor.section_fields(EditorCategory::Network).len();
        editor.focus = network_count - 1;
        editor.category = EditorCategory::Network;

        editor.move_focus(1);

        assert_eq!(editor.category, EditorCategory::Network);
        assert_eq!(editor.current_field(), Field::ConnectionName);

        editor.move_focus(-1);

        assert_eq!(editor.category, EditorCategory::Network);
        assert_eq!(editor.current_field(), Field::Hidden);
    }

    #[test]
    fn manual_ipv4_lists_each_address_and_add_row() {
        let mut editor = editor_state();
        editor.draft.ipv4.method = IpMethod::Manual;
        editor.draft.ipv4.addresses = vec![
            IpAddress {
                address: "192.0.2.10".to_string(),
                prefix: 24,
            },
            IpAddress {
                address: "192.0.2.11".to_string(),
                prefix: 24,
            },
        ];
        editor.category = EditorCategory::Ipv4;

        let fields = editor.section_fields(EditorCategory::Ipv4);

        assert!(fields.contains(&Field::Ipv4Address(0)));
        assert!(fields.contains(&Field::Ipv4Address(1)));
        assert!(fields.contains(&Field::Ipv4AddAddress));
        assert_eq!(editor.field_span(Field::Ipv4Address(1)).1, 1);
    }

    #[test]
    fn adding_an_address_focuses_its_new_row() {
        let mut editor = editor_state();
        editor.draft.ipv4.method = IpMethod::Manual;

        activate_field(&mut editor, Field::Ipv4AddAddress);

        assert_eq!(editor.draft.ipv4.addresses.len(), 1);
        assert_eq!(editor.current_field(), Field::Ipv4Address(0));
        assert_eq!(
            editor.input.as_ref().map(|input| input.field),
            Some(Field::Ipv4Address(0))
        );
    }

    #[test]
    fn clearing_an_address_removes_its_row() {
        let mut editor = editor_state();
        editor.draft.ipv4.method = IpMethod::Manual;
        editor.draft.ipv4.addresses = vec![IpAddress {
            address: "192.0.2.10".to_string(),
            prefix: 24,
        }];

        commit_input(&mut editor, Field::Ipv4Address(0), "").unwrap();

        assert!(editor.draft.ipv4.addresses.is_empty());
    }

    #[test]
    fn tab_and_shift_tab_jump_sections() {
        let mut editor = editor_state();
        editor.focus = 4;

        editor.move_section(1);
        assert_eq!(editor.category, EditorCategory::Ipv4);
        assert_eq!(editor.current_field(), Field::Ipv4Method);

        editor.move_section(1);
        assert_eq!(editor.category, EditorCategory::Ipv6);
        assert_eq!(editor.current_field(), Field::Ipv6Method);

        editor.move_section(-1);
        assert_eq!(editor.category, EditorCategory::Ipv4);
        assert_eq!(editor.current_field(), Field::Ipv4Method);
    }

    #[test]
    fn selected_span_covers_side_by_side_field() {
        let editor = editor_state();
        let (top, height) = editor.selected_span();

        assert_eq!(top, 2);
        assert_eq!(height, 1);
    }

    #[test]
    fn section_rows_group_fields_without_adding_focus_targets() {
        let editor = editor_state();
        let rows = editor.section_rows(EditorCategory::Network);

        assert!(matches!(rows.first(), Some(SectionRow::Space)));
        assert!(matches!(rows.get(1), Some(SectionRow::Group("General"))));
        assert_eq!(editor.field_span(Field::ConnectionName), (2, 1));
        assert_eq!(editor.field_span(Field::Ssid), (8, 1));
        assert_eq!(editor.section_fields(EditorCategory::Network).len(), 13);
    }

    #[test]
    fn selected_span_includes_open_select_popup() {
        let mut editor = editor_state();
        editor.focus_field(Field::Security);
        editor.select = Some(SelectState {
            field: Field::Security,
            selected: 0,
        });
        let (top, height) = editor.selected_span();

        assert_eq!(top, editor.field_span(Field::Security).0);
        assert_eq!(height, 1 + popup_height(&editor, Field::Security));
    }

    #[test]
    fn window_characters_keeps_cursor_visible() {
        let mut characters = "abcdefghijklmnopqrstuvwxyz".chars().collect::<Vec<_>>();
        characters.insert(20, '▏');

        let value = window_characters(&characters, 20, 10);

        assert_eq!(value.chars().count(), 10);
        assert!(value.contains('▏'));
        assert!(value.starts_with('…'));
    }

    #[test]
    fn show_password_reveals_pending_input_only() {
        let mut editor = editor_state();
        editor.input = Some(InputState {
            field: Field::Password,
            value: "secret".to_string(),
            cursor: 6,
        });

        assert_eq!(display_value(&editor, Field::Password), "******▏");
        editor.show_password = true;
        assert_eq!(display_value(&editor, Field::Password), "secret▏");
    }
}
