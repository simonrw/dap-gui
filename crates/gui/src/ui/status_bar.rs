use std::collections::VecDeque;
use std::time::Instant;

use eframe::egui::{self, Color32, RichText, Widget};

const MAX_NOTIFICATIONS: usize = 5;
const NOTIFICATION_DURATION_SECS: f64 = 8.0;

#[derive(Clone)]
pub(crate) struct Notification {
    pub message: String,
    pub level: NotificationLevel,
    pub created: Instant,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum NotificationLevel {
    Info,
    Error,
}

pub(crate) struct StatusState {
    pub notifications: VecDeque<Notification>,
    pub last_error: Option<String>,
}

impl Default for StatusState {
    fn default() -> Self {
        Self {
            notifications: VecDeque::new(),
            last_error: None,
        }
    }
}

impl StatusState {
    pub fn push_error(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.last_error = Some(msg.clone());
        self.push_notification(msg, NotificationLevel::Error);
    }

    pub fn push_info(&mut self, message: impl Into<String>) {
        self.push_notification(message.into(), NotificationLevel::Info);
    }

    fn push_notification(&mut self, message: String, level: NotificationLevel) {
        self.notifications.push_back(Notification {
            message,
            level,
            created: Instant::now(),
        });
        if self.notifications.len() > MAX_NOTIFICATIONS {
            self.notifications.pop_front();
        }
    }

    pub fn gc(&mut self) {
        let now = Instant::now();
        self.notifications
            .retain(|n| now.duration_since(n.created).as_secs_f64() < NOTIFICATION_DURATION_SECS);
    }
}

pub(crate) struct StatusBar<'a> {
    state_label: &'a str,
    status: &'a mut StatusState,
}

impl<'a> StatusBar<'a> {
    pub fn new(state_label: &'a str, status: &'a mut StatusState) -> Self {
        Self {
            state_label,
            status,
        }
    }
}

impl Widget for StatusBar<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.status.gc();

        ui.horizontal(|ui| {
            ui.label(RichText::new(self.state_label).strong());

            ui.separator();

            // Show most recent notification (if any)
            if let Some(notification) = self.status.notifications.back() {
                let color = match notification.level {
                    NotificationLevel::Info => ui.visuals().text_color(),
                    NotificationLevel::Error => Color32::from_rgb(255, 80, 80),
                };
                ui.label(RichText::new(&notification.message).color(color));
            }
        })
        .response
    }
}
