#[cfg(feature = "album-art")]
use image::DynamicImage;

use crate::dbus::{DBusConnection, DBusError};
use crate::formatter::FormattedNotification;
use crate::mpris::PlayerMetadata;
use crate::Configuration;
use rustbus::MessageBuilder;
use rustbus::{dbus_variant_sig, Marshal, Signature, Unmarshal};
use std::collections::HashMap;

const NOTIFICATION_NAMESPACE: &str = "org.freedesktop.Notifications";
const NOTIFICATION_OBJECTPATH: &str = "/org/freedesktop/Notifications";
const NOTIFICATION_SOURCE: &str = "mpris-notifier";

pub struct Notifier {
    configuration: Configuration,
}

#[derive(Debug)]
pub struct Notification {
    sender: String,
    metadata: PlayerMetadata,
    album_art: Option<NotificationImage>,
}

impl Notification {
    pub fn new(
        sender: &str,
        metadata: &PlayerMetadata,
        album_art: Option<NotificationImage>,
    ) -> Self {
        Self {
            sender: sender.to_string(),
            metadata: metadata.clone(),
            album_art,
        }
    }

    // Updates an existing notification with new metadata or album art.
    pub fn update(&mut self, metadata: &PlayerMetadata, album_art: Option<NotificationImage>) {
        self.metadata = metadata.clone();
        self.album_art = album_art;
    }

    pub fn sender(&self) -> &str {
        &self.sender
    }
}

type NotificationHintMap = HashMap<String, NotificationHintVariant>;
dbus_variant_sig!(NotificationHintVariant, CaseString => String; CaseNotificationImage => NotificationImage; Urgency => u8; Category => String);

// See: https://specifications.freedesktop.org/notification-spec/notification-spec-latest.html#icons-and-images
#[derive(Marshal, Unmarshal, Signature, Debug, Eq, PartialEq, Clone)]
pub struct NotificationImage {
    width: i32,
    height: i32,
    rowstride: i32,
    alpha: bool,
    bits_per_sample: i32,
    channels: i32,
    data: Vec<u8>,
}

#[cfg(feature = "album-art")]
impl From<DynamicImage> for NotificationImage {
    fn from(image: DynamicImage) -> Self {
        let has_alpha = image.color() == image::ColorType::Rgba8;
        let channels = if has_alpha { 4 } else { 3 };

        Self {
            width: image.width() as i32,
            height: image.height() as i32,
            rowstride: (image.width() * channels) as i32,
            alpha: has_alpha,
            bits_per_sample: 8,
            channels: channels as i32,
            data: image.into_bytes(),
        }
    }
}

impl Notifier {
    pub fn new(configuration: &Configuration) -> Self {
        Self {
            configuration: configuration.clone(),
        }
    }

    pub fn send_notification(
        &self,
        notification: Notification,
        dbus: &mut DBusConnection,
    ) -> Result<(), DBusError> {
        let metadata = &notification.metadata;
        let album_art = notification.album_art;

        // See: https://github.com/hoodie/notify-rust/blob/main/src/xdg/dbus_rs.rs#L64-L73
        let mut message = MessageBuilder::new()
            .call("Notify")
            .at(NOTIFICATION_NAMESPACE)
            .on(NOTIFICATION_OBJECTPATH)
            .with_interface(NOTIFICATION_NAMESPACE)
            .build();

        let subject = self.format_metadata(&self.configuration.subject_format, metadata);
        let body = self.format_metadata(&self.configuration.body_format, metadata);

        if subject.trim().is_empty() && body.trim().is_empty() {
            // Don't bother popping an empty notification window up
            return Ok(());
        }

        message.body.push_param(NOTIFICATION_SOURCE)?; // appname (TODO)
        message.body.push_param(0_u32)?; // update ID
        message.body.push_param("")?; // icon
        message.body.push_param(subject)?; // summary
        message.body.push_param(body)?; // body
        message.body.push_param(Vec::<String>::new())?; // actions (array of strings)
        let mut hints: NotificationHintMap = HashMap::new();
        hints.insert(
            "x-canonical-private-synchronous".to_string(),
            NotificationHintVariant::CaseString(NOTIFICATION_SOURCE.to_string()),
        );
        if let Some(album_art) = album_art {
            hints.insert(
                "image-data".to_string(),
                NotificationHintVariant::CaseNotificationImage(album_art),
            );
        }
        if let Some(urgency) = self.configuration.urgency {
            hints.insert(
                "urgency".to_string(),
                NotificationHintVariant::Urgency(urgency),
            );
        }
        if let Some(category) = &self.configuration.category {
            hints.insert(
                "category".to_string(),
                NotificationHintVariant::Category(category.clone()),
            );
        }
        message.body.push_param(&hints)?; // hints (dict of a{sv})
        message.body.push_param(-1_i32)?; // timeout

        dbus.send_message(&message)
    }

    // Very permissive parsing algorithm (markup).
    fn format_metadata(&self, fmt: &str, metadata: &PlayerMetadata) -> String {
        FormattedNotification::new(fmt, metadata, &self.configuration.join_string).to_string()
    }
}
