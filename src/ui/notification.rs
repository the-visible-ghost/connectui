use std::time::{Duration, Instant};

use iocraft::prelude::*;

const MAX_WIDTH: u32 = 44;
const MAX_CONTENT_WIDTH: usize = 38;
const MAX_MESSAGE_LINES: usize = 3;
const CHROME_WIDTH: u32 = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl NotificationLevel {
    fn duration(self) -> Duration {
        match self {
            Self::Error => Duration::from_secs(7),
            Self::Warning => Duration::from_secs(5),
            Self::Info | Self::Success => Duration::from_secs(4),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Info => "󰋼",
            Self::Success => "󰄬",
            Self::Warning => "󰀪",
            Self::Error => "󰅚",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Info => Color::Blue,
            Self::Success => Color::Green,
            Self::Warning => Color::Yellow,
            Self::Error => Color::Red,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Notification {
    level: NotificationLevel,
    title: String,
    message: String,
    expires: Instant,
}

impl Notification {
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut value: String = value.chars().take(max.saturating_sub(1)).collect();
    if max > 0 {
        value.push('…');
    }
    value
}

fn wrap_lines(value: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in value.split_whitespace() {
        let length = word.chars().count();
        if length > width {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            let mut chunk = String::new();
            for character in word.chars() {
                chunk.push(character);
                if chunk.chars().count() == width {
                    lines.push(std::mem::take(&mut chunk));
                }
            }
            current = chunk;
        } else if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + length <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn wrap_message(message: &str, width: usize, max_lines: usize) -> Vec<String> {
    let mut lines = wrap_lines(message, width);
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            *last = truncate(last, width.saturating_sub(1));
            last.push('…');
        }
    }
    lines
}

pub fn notify(
    mut notifications: State<Vec<Notification>>,
    level: NotificationLevel,
    title: impl Into<String>,
    message: impl Into<String>,
) {
    let title = title.into();
    let message = message.into();
    let mut items = (*notifications.read()).clone();
    items.push(Notification {
        level,
        title: truncate(&title, 40),
        message: truncate(&message, 180),
        expires: Instant::now() + level.duration(),
    });
    if items.len() > 4 {
        items.remove(0);
    }
    notifications.set(items);
}

fn notification_card(notification: &Notification) -> AnyElement<'static> {
    let accent = notification.level.color();
    let title = truncate(&notification.title, MAX_CONTENT_WIDTH);
    let natural_message_width = notification
        .message
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let target = title
        .chars()
        .count()
        .max(natural_message_width.min(MAX_CONTENT_WIDTH))
        .max(1);
    let message_lines = wrap_message(&notification.message, target, MAX_MESSAGE_LINES);
    let message_width = message_lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let content_width = title
        .chars()
        .count()
        .max(message_width)
        .clamp(1, MAX_CONTENT_WIDTH);

    element! {
        View(
            width: content_width as u32 + CHROME_WIDTH,
            border_style: BorderStyle::Round,
            border_color: accent,
            background_color: Color::Reset,
            padding_left: 1,
            padding_right: 1,
            gap: 1,
        ) {
            View(
                position: Position::Absolute,
                top: -1,
                left: 0,
                right: 0,
                justify_content: JustifyContent::Center,
            ) {
                View(
                    background_color: Color::Reset,
                    padding_left: 1,
                    padding_right: 1,
                ) {
                    Text(
                        content: title,
                        color: accent,
                        weight: Weight::Bold,
                        wrap: TextWrap::NoWrap,
                    )
                }
            }
            Text(
                content: notification.level.icon(),
                color: accent,
                weight: Weight::Bold,
            )
            Text(
                content: message_lines.join("\n"),
                color: Color::Reset,
                wrap: TextWrap::NoWrap,
            )
        }
    }
    .into_any()
}

pub fn notification_overlay(notifications: &[Notification]) -> AnyElement<'static> {
    let cards: Vec<AnyElement<'static>> =
        notifications.iter().rev().map(notification_card).collect();

    element! {
        View(
            position: Position::Absolute,
            top: 0,
            right: 0,
            width: MAX_WIDTH,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            gap: 0,
        ) {
            #(cards)
        }
    }
    .into_any()
}
