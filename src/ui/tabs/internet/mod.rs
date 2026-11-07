mod connection;
mod editor;
mod networks;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::network::{ConnectOutcome, KnownProfile, NMClient, ProfileDraft, Security, Snapshot};
use crate::ui::common::{Props, Theme};
use crate::ui::notification::{Notification, NotificationLevel, notify};
use connection::connection_panel;
use editor::{EditorAction, EditorState, editor_overlay, editor_panel, footer_hint, handle_key};
use iocraft::components::ScrollViewHandle;
use iocraft::prelude::*;
use networks::{NetworksView, PendingConnect, networks_panel, spinner_frame};
use smol::channel::{Receiver, Sender, unbounded};
use zbus::OwnedObjectPath;

#[derive(Clone, Copy)]
#[repr(i8)]
enum Menu {
    Connection = 0,
    Network = 1,
}

impl std::convert::From<i8> for Menu {
    fn from(value: i8) -> Self {
        match value {
            1 => Self::Network,
            _ => Self::Connection,
        }
    }
}

enum Command {
    Scan {
        device: OwnedObjectPath,
    },
    Connect {
        device: OwnedObjectPath,
        access_point: OwnedObjectPath,
        ssid: String,
        security: Security,
        password: Option<String>,
    },
    Disconnect {
        active: OwnedObjectPath,
    },
    OpenEditor {
        ssid: String,
        security: Security,
        interface: String,
    },
    SaveProfile {
        draft: Box<ProfileDraft>,
    },
}

#[derive(Clone)]
struct Channels {
    tx: Sender<Command>,
    rx: Arc<Receiver<Command>>,
}

fn snapshot_copy(snapshot: State<Snapshot>) -> Snapshot {
    (*snapshot.read()).clone()
}

fn apply_known(snapshot: &mut Snapshot, known_profiles: &HashMap<String, KnownProfile>) {
    let Some(wifi) = snapshot.wifi.as_mut() else {
        return;
    };
    for network in &mut wifi.networks {
        if let Some(profile) = known_profiles.get(&network.ssid) {
            network.known = true;
            network.hidden = profile.hidden;
            network.auto_connect = profile.auto_connect;
        } else {
            network.known = false;
            network.hidden = false;
            network.auto_connect = false;
        }
    }
    wifi.networks.sort_by(|left, right| {
        right
            .known
            .cmp(&left.known)
            .then_with(|| right.active.cmp(&left.active))
            .then_with(|| right.strength.cmp(&left.strength))
            .then_with(|| left.ssid.cmp(&right.ssid))
    });
}

fn step_selection(snapshot: &Snapshot, mut selected_index: State<usize>, delta: isize) {
    let count = snapshot
        .wifi
        .as_ref()
        .map(|wifi| wifi.networks.len())
        .unwrap_or(0);
    if count == 0 {
        return;
    }
    let current = selected_index.get().min(count - 1);
    let next = (current as isize + delta).rem_euclid(count as isize) as usize;
    selected_index.set(next);
}

fn password_modal(theme: Theme, prompt: &PendingConnect, password: &str) -> AnyElement<'static> {
    let ssid: String = prompt.ssid.chars().take(40).collect();
    let masked = "*".repeat(password.chars().count());
    let cursor = if password.is_empty() { "_" } else { "" };
    let display: String = format!("{masked}{cursor}").chars().take(40).collect();

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
                        content: "󰌾 PASSWORD",
                        color: theme.primary,
                        weight: Weight::Bold,
                    )
                }
                View(width: 100pct, justify_content: JustifyContent::Center) {
                    Text(
                        content: format!("{ssid} · {}", prompt.security.label()),
                        color: theme.secondary,
                    )
                }
                View(
                    width: 100pct,
                    border_style: BorderStyle::Round,
                    border_edges: Edges::Bottom,
                    border_color: theme.secondary_dark,
                    padding_left: 1,
                    padding_right: 1,
                ) {
                    Text(content: display, color: Color::White)
                }
                View(width: 100pct, justify_content: JustifyContent::Center) {
                    Text(content: "Enter connect · Esc cancel", color: Color::Grey)
                }
            }
        }
    }
    .into_any()
}

#[component]
pub fn InternetContent(mut hooks: Hooks, props: &Props) -> impl Into<AnyElement<'static>> {
    let mut menu = hooks.use_state(|| Menu::Connection);
    let mut selected_index = hooks.use_state(|| 0usize);
    let mut g_pressed = hooks.use_state(|| false);
    let mut snapshot = hooks.use_state(Snapshot::default);
    let mut scanning = hooks.use_state(|| false);
    let mut busy = hooks.use_state(|| false);
    let mut connecting = hooks.use_state(|| false);
    let mut spinner = hooks.use_state(|| 0usize);
    let mut error = hooks.use_state(|| None::<String>);
    let mut pending = hooks.use_state(|| None::<PendingConnect>);
    let mut password = hooks.use_state(String::new);
    let local_modal = hooks.use_state(|| false);
    let mut modal_active = props.modal_active.unwrap_or(local_modal);
    let local_notifications = hooks.use_state(Vec::<Notification>::new);
    let notifications = props.notifications.unwrap_or(local_notifications);
    let mut footer_override = props.footer;
    let mut editor = hooks.use_state(|| None::<EditorState>);
    let scroll = hooks.use_ref_default::<ScrollViewHandle>();
    let editor_scroll = hooks.use_ref_default::<ScrollViewHandle>();
    let channels = hooks.use_state(|| {
        let (tx, rx) = unbounded::<Command>();
        Channels {
            tx,
            rx: Arc::new(rx),
        }
    });
    let (terminal_width, terminal_height) = hooks.use_terminal_size();
    let (tx, rx) = {
        let channels = channels.read();
        (channels.tx.clone(), Arc::clone(&channels.rx))
    };

    hooks.use_future(async move {
        loop {
            smol::Timer::after(Duration::from_millis(100)).await;
            if busy.get() || scanning.get() {
                spinner.set(spinner.get().wrapping_add(1));
            }
        }
    });

    hooks.use_future(async move {
        let client = match NMClient::new().await {
            Ok(client) => client,
            Err(error_value) => {
                let message = error_value.to_string();
                error.set(Some(message.clone()));
                notify(notifications, NotificationLevel::Error, "Wi-Fi", message);
                return;
            }
        };
        let mut known_profiles = client.known_profiles().await.unwrap_or_default();
        let mut scanning_until: Option<Instant> = None;
        let mut first_refresh = true;

        loop {
            match client.snapshot().await {
                Ok(mut value) => {
                    apply_known(&mut value, &known_profiles);
                    let device = value.wifi.as_ref().map(|wifi| wifi.device_path.clone());
                    snapshot.set(value);
                    if first_refresh {
                        first_refresh = false;
                        if let Some(device) = device {
                            match client.request_scan(&device).await {
                                Ok(()) => {
                                    scanning.set(true);
                                    scanning_until = Some(Instant::now() + Duration::from_secs(3));
                                    notify(
                                        notifications,
                                        NotificationLevel::Info,
                                        "Scan",
                                        "Scanning for networks",
                                    );
                                }
                                Err(error_value) => {
                                    let message = error_value.to_string();
                                    error.set(Some(message.clone()));
                                    notify(
                                        notifications,
                                        NotificationLevel::Error,
                                        "Scan failed",
                                        message,
                                    );
                                }
                            }
                        }
                    }
                }
                Err(error_value) => {
                    let message = error_value.to_string();
                    let changed = (*error.read()).as_deref() != Some(message.as_str());
                    error.set(Some(message.clone()));
                    if changed {
                        notify(notifications, NotificationLevel::Error, "Wi-Fi", message);
                    }
                }
            }

            if scanning_until.is_some_and(|deadline| Instant::now() >= deadline) {
                scanning_until = None;
                scanning.set(false);
                let count = snapshot
                    .read()
                    .wifi
                    .as_ref()
                    .map(|wifi| wifi.networks.len())
                    .unwrap_or(0);
                notify(
                    notifications,
                    NotificationLevel::Info,
                    "Scan",
                    format!("Scan complete · {count} networks"),
                );
            }

            let timeout = if scanning_until.is_some() {
                Duration::from_millis(700)
            } else {
                Duration::from_secs(2)
            };
            let received = {
                let receive = async { Some(rx.recv().await) };
                let sleep = async {
                    smol::Timer::after(timeout).await;
                    None
                };
                smol::future::or(receive, sleep).await
            };

            match received {
                Some(Ok(command)) => {
                    error.set(None);
                    match command {
                        Command::Scan { device } => {
                            match client.request_scan(&device).await {
                                Ok(()) => {
                                    scanning.set(true);
                                    scanning_until = Some(Instant::now() + Duration::from_secs(3));
                                    notify(
                                        notifications,
                                        NotificationLevel::Info,
                                        "Scan",
                                        "Scanning for networks",
                                    );
                                }
                                Err(error_value) => {
                                    let message = error_value.to_string();
                                    error.set(Some(message.clone()));
                                    notify(
                                        notifications,
                                        NotificationLevel::Error,
                                        "Scan failed",
                                        message,
                                    );
                                }
                            }
                            if let Ok(updated) = client.known_profiles().await {
                                known_profiles = updated;
                            }
                        }
                        Command::Connect {
                            device,
                            access_point,
                            ssid,
                            security,
                            password: entered_password,
                        } => {
                            busy.set(true);
                            connecting.set(true);
                            notify(
                                notifications,
                                NotificationLevel::Info,
                                "Connection",
                                format!("Connecting to {ssid}"),
                            );
                            let refresh_known = match client
                                .connect(
                                    &device,
                                    &access_point,
                                    &ssid,
                                    security,
                                    entered_password.as_deref(),
                                )
                                .await
                            {
                                Ok(ConnectOutcome::NeedsPassword) => {
                                    notify(
                                        notifications,
                                        NotificationLevel::Warning,
                                        "Password required",
                                        ssid.clone(),
                                    );
                                    pending.set(Some(PendingConnect { ssid, security }));
                                    password.set(String::new());
                                    modal_active.set(true);
                                    false
                                }
                                Ok(ConnectOutcome::WrongPassword) => {
                                    notify(
                                        notifications,
                                        NotificationLevel::Warning,
                                        "Wrong password",
                                        format!("Re-enter password for {ssid}"),
                                    );
                                    pending.set(Some(PendingConnect { ssid, security }));
                                    password.set(String::new());
                                    modal_active.set(true);
                                    true
                                }
                                Ok(ConnectOutcome::Started) => {
                                    notify(
                                        notifications,
                                        NotificationLevel::Success,
                                        "Connected",
                                        ssid,
                                    );
                                    true
                                }
                                Err(error_value) => {
                                    let message = error_value.to_string();
                                    error.set(Some(message.clone()));
                                    notify(
                                        notifications,
                                        NotificationLevel::Error,
                                        "Connection failed",
                                        message,
                                    );
                                    true
                                }
                            };
                            if refresh_known && let Ok(updated) = client.known_profiles().await {
                                known_profiles = updated;
                            }
                            connecting.set(false);
                            busy.set(false);
                        }
                        Command::Disconnect { active } => {
                            busy.set(true);
                            notify(
                                notifications,
                                NotificationLevel::Info,
                                "Connection",
                                "Disconnecting",
                            );
                            match client.disconnect(&active).await {
                                Ok(()) => notify(
                                    notifications,
                                    NotificationLevel::Success,
                                    "Disconnected",
                                    "Wi-Fi disconnected",
                                ),
                                Err(error_value) => {
                                    let message = error_value.to_string();
                                    error.set(Some(message.clone()));
                                    notify(
                                        notifications,
                                        NotificationLevel::Error,
                                        "Disconnect failed",
                                        message,
                                    );
                                }
                            }
                            busy.set(false);
                        }
                        Command::OpenEditor {
                            ssid,
                            security,
                            interface,
                        } => {
                            let loaded = match client.find_saved_connection(&ssid).await {
                                Ok(Some(path)) => {
                                    client.load_profile(&path).await.map(|draft| (draft, false))
                                }
                                Ok(None) => {
                                    Ok((ProfileDraft::new_wifi(&ssid, security, &interface), true))
                                }
                                Err(error_value) => Err(error_value),
                            };
                            match loaded {
                                Ok((draft, is_new)) => {
                                    let devices = client.devices().await.unwrap_or_default();
                                    editor.set(Some(EditorState::new(draft, devices, is_new)));
                                    modal_active.set(true);
                                    if let Some(footer) = footer_override.as_mut() {
                                        footer.set(Some(footer_hint(u32::from(terminal_width))));
                                    }
                                }
                                Err(error_value) => {
                                    let message = error_value.to_string();
                                    error.set(Some(message.clone()));
                                    notify(
                                        notifications,
                                        NotificationLevel::Error,
                                        "Edit failed",
                                        message,
                                    );
                                }
                            }
                        }
                        Command::SaveProfile { draft } => {
                            let profile = draft.id.clone();
                            match client.save_profile(&draft).await {
                                Ok(()) => {
                                    if let Ok(updated) = client.known_profiles().await {
                                        known_profiles = updated;
                                    }
                                    editor.set(None);
                                    modal_active.set(false);
                                    if let Some(footer) = footer_override.as_mut() {
                                        footer.set(None);
                                    }
                                    notify(
                                        notifications,
                                        NotificationLevel::Success,
                                        "Profile saved",
                                        profile,
                                    );
                                }
                                Err(error_value) => {
                                    let message = error_value.to_string();
                                    let mut current = editor.read().clone().unwrap_or_else(|| {
                                        EditorState::new((*draft).clone(), Vec::new(), false)
                                    });
                                    current.saving = false;
                                    current.error = Some(message.clone());
                                    editor.set(Some(current));
                                    notify(
                                        notifications,
                                        NotificationLevel::Error,
                                        "Save failed",
                                        message,
                                    );
                                }
                            }
                        }
                    }
                }
                Some(Err(_)) => break,
                None => {}
            }
        }
    });

    let current = snapshot_copy(snapshot);
    let network_list = current
        .wifi
        .as_ref()
        .map(|wifi| wifi.networks.as_slice())
        .unwrap_or(&[]);
    let network_count = network_list.len();
    let current_index = if network_count == 0 {
        0
    } else {
        selected_index.get().min(network_count - 1)
    };
    let selected_row = networks::selected_row(network_list, current_index);
    let table_width = u32::from(terminal_width).saturating_sub(12).max(20);
    let mut scroll_handle = scroll;
    hooks.use_effect(
        move || {
            if network_count == 0 {
                return;
            }
            let viewport = scroll_handle.read().viewport_height() as usize;
            if viewport == 0 {
                return;
            }
            let offset = scroll_handle.read().scroll_offset().max(0) as usize;
            if selected_row < offset {
                scroll_handle.write().scroll_to(selected_row as i32);
            } else if selected_row >= offset + viewport {
                scroll_handle
                    .write()
                    .scroll_to((selected_row + 1 - viewport) as i32);
            }
        },
        (selected_row, network_count, terminal_height),
    );

    let current_editor = (*editor.read()).clone();
    let editor_row = current_editor
        .as_ref()
        .map(EditorState::selected_row)
        .unwrap_or(0);
    let editor_open = current_editor.is_some();
    let mut editor_scroll_handle = editor_scroll;
    hooks.use_effect(
        move || {
            if !editor_open {
                return;
            }
            let viewport = editor_scroll_handle.read().viewport_height() as usize;
            if viewport == 0 {
                return;
            }
            let offset = editor_scroll_handle.read().scroll_offset().max(0) as usize;
            if editor_row < offset {
                editor_scroll_handle.write().scroll_to(editor_row as i32);
            } else if editor_row >= offset + viewport {
                editor_scroll_handle
                    .write()
                    .scroll_to((editor_row + 1 - viewport) as i32);
            }
        },
        (editor_row, editor_open, terminal_height),
    );

    hooks.use_terminal_events({
        move |event| {
            let TerminalEvent::Key(KeyEvent { code, kind, .. }) = event else {
                return;
            };
            if kind == KeyEventKind::Release {
                return;
            }

            let pending_connect = (*pending.read()).clone();
            if let Some(connect) = pending_connect {
                if g_pressed.get() {
                    g_pressed.set(false);
                }
                match code {
                    KeyCode::Esc => {
                        notify(
                            notifications,
                            NotificationLevel::Info,
                            "Password",
                            "Connection cancelled",
                        );
                        pending.set(None);
                        password.set(String::new());
                        modal_active.set(false);
                    }
                    KeyCode::Enter => {
                        let entered = (*password.read()).clone();
                        if entered.is_empty() {
                            return;
                        }
                        if let Some(wifi) = snapshot_copy(snapshot).wifi
                            && let Some(network) = wifi
                                .networks
                                .iter()
                                .find(|network| network.ssid == connect.ssid)
                        {
                            let command = Command::Connect {
                                device: wifi.device_path.clone(),
                                access_point: network.ap_path.clone(),
                                ssid: network.ssid.clone(),
                                security: connect.security,
                                password: Some(entered),
                            };
                            if tx.try_send(command).is_ok() {
                                busy.set(true);
                                connecting.set(true);
                            }
                        }
                        pending.set(None);
                        password.set(String::new());
                        modal_active.set(false);
                    }
                    KeyCode::Backspace => {
                        let mut value = (*password.read()).clone();
                        value.pop();
                        password.set(value);
                    }
                    KeyCode::Char(character) => {
                        let mut value = (*password.read()).clone();
                        value.push(character);
                        password.set(value);
                    }
                    _ => {}
                }
                return;
            }

            let active_editor = (*editor.read()).clone();
            if let Some(mut current) = active_editor {
                if g_pressed.get() {
                    g_pressed.set(false);
                }
                let was_dirty = current.dirty();
                match handle_key(&mut current, code) {
                    EditorAction::None => editor.set(Some(current)),
                    EditorAction::Close => {
                        editor.set(None);
                        modal_active.set(false);
                        if let Some(footer) = footer_override.as_mut() {
                            footer.set(None);
                        }
                        if was_dirty {
                            notify(
                                notifications,
                                NotificationLevel::Info,
                                "Profile editor",
                                "Changes discarded",
                            );
                        }
                    }
                    EditorAction::Save(draft) => {
                        editor.set(Some(current));
                        if tx.try_send(Command::SaveProfile { draft }).is_err() {
                            let mut state = editor.read().clone();
                            if let Some(state) = state.as_mut() {
                                state.saving = false;
                                state.error = Some("Could not save the profile".to_string());
                            }
                            editor.set(state);
                        }
                    }
                }
                return;
            }

            if code == KeyCode::Char('g') {
                if g_pressed.get() {
                    selected_index.set(0);
                    g_pressed.set(false);
                } else {
                    g_pressed.set(true);
                }
                return;
            }
            if g_pressed.get() {
                g_pressed.set(false);
            }

            match code {
                KeyCode::Tab => menu.set(((menu.get() as i8) + 1).rem_euclid(2).into()),
                KeyCode::BackTab => menu.set(((menu.get() as i8) - 1).rem_euclid(2).into()),
                KeyCode::Char('j') | KeyCode::Down => {
                    step_selection(&snapshot_copy(snapshot), selected_index, 1)
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    step_selection(&snapshot_copy(snapshot), selected_index, -1)
                }
                KeyCode::Char('G') => {
                    let count = snapshot_copy(snapshot)
                        .wifi
                        .as_ref()
                        .map(|wifi| wifi.networks.len())
                        .unwrap_or(0);
                    if count > 0 {
                        selected_index.set(count - 1);
                    }
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    if busy.get() {
                        return;
                    }
                    let Some(wifi) = snapshot_copy(snapshot).wifi else {
                        return;
                    };
                    let target = if code == KeyCode::Char('E') {
                        let Some(active) = wifi.active.as_ref() else {
                            notify(
                                notifications,
                                NotificationLevel::Info,
                                "Profile editor",
                                "No active connection",
                            );
                            return;
                        };
                        (active.ssid.clone(), active.security)
                    } else {
                        let index = selected_index
                            .get()
                            .min(wifi.networks.len().saturating_sub(1));
                        let Some(network) = wifi.networks.get(index) else {
                            return;
                        };
                        (network.ssid.clone(), network.security)
                    };
                    let command = Command::OpenEditor {
                        ssid: target.0,
                        security: target.1,
                        interface: wifi.interface.clone(),
                    };
                    if tx.try_send(command).is_err() {
                        notify(
                            notifications,
                            NotificationLevel::Error,
                            "Profile editor",
                            "Could not open the editor",
                        );
                    }
                }
                KeyCode::Char('s') => {
                    if busy.get() || scanning.get() {
                        return;
                    }
                    if let Some(wifi) = snapshot_copy(snapshot).wifi {
                        error.set(None);
                        let _ = tx.try_send(Command::Scan {
                            device: wifi.device_path,
                        });
                    }
                }
                KeyCode::Char(' ') | KeyCode::Enter => {
                    if busy.get() {
                        return;
                    }
                    let Some(wifi) = snapshot_copy(snapshot).wifi else {
                        return;
                    };
                    let index = selected_index
                        .get()
                        .min(wifi.networks.len().saturating_sub(1));
                    let Some(network) = wifi.networks.get(index) else {
                        return;
                    };
                    if network.active {
                        let Some(active) = wifi.active else {
                            return;
                        };
                        if tx
                            .try_send(Command::Disconnect {
                                active: active.active_path,
                            })
                            .is_ok()
                        {
                            busy.set(true);
                        }
                    } else {
                        let command = Command::Connect {
                            device: wifi.device_path.clone(),
                            access_point: network.ap_path.clone(),
                            ssid: network.ssid.clone(),
                            security: network.security,
                            password: None,
                        };
                        if tx.try_send(command).is_ok() {
                            busy.set(true);
                            connecting.set(true);
                        }
                    }
                }
                _ => {}
            }
        }
    });

    let theme = props.theme;
    let focused = menu.get();
    let prompt = (*pending.read()).clone();
    let password_value = (*password.read()).clone();
    let wifi = current.wifi.as_ref();
    let spinner_value = spinner_frame(spinner.get());
    let activity_status = if connecting.get() {
        Some("Connecting")
    } else if busy.get() {
        Some("Disconnecting")
    } else {
        None
    };
    let table_status = if activity_status.is_some() {
        activity_status
    } else if scanning.get() {
        Some("Scanning")
    } else {
        None
    };
    let editor_view = current_editor
        .as_ref()
        .map(|editor| editor_panel(theme, editor, u32::from(terminal_width), editor_scroll));
    let editor_overlay_view = current_editor
        .as_ref()
        .and_then(|editor| editor_overlay(theme, editor));
    let normal_panel: Option<AnyElement<'static>> = current_editor.is_none().then(|| {
        element! {
            View(
                flex_grow: 1.0,
                width: 100pct,
                flex_direction: FlexDirection::Column,
            ) {
                #(Some(connection_panel(
                    theme,
                    matches!(focused, Menu::Connection),
                    wifi,
                    activity_status,
                    spinner_value,
                )))
                #(Some(networks_panel(NetworksView {
                    theme,
                    focused: matches!(focused, Menu::Network),
                    networks: network_list,
                    selected: current_index,
                    table_width,
                    status: table_status,
                    spinner: spinner_value,
                    handle: scroll,
                })))
            }
        }
        .into_any()
    });

    element! {
        View(
            flex_grow: 1.0,
            width: 100pct,
            flex_direction: FlexDirection::Column,
        ) {
            #(normal_panel)
            #(editor_view)
            #(editor_overlay_view)
            #(prompt.as_ref().map(|prompt| {
                password_modal(theme, prompt, &password_value)
            }))
        }
    }
}
