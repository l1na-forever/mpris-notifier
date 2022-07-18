use crate::dbus::{DBusConnection, DBusError};
use crate::formatter::FormattedNotification;
use crate::mpris::{PlayerMetadata, PlayerStatus};
use crate::Configuration;
use rustbus::message_builder::MarshalledMessage;
use rustbus::MessageBuilder;
use rustbus::{Marshal, Signature, Unmarshal};
use std::collections::HashMap;

const NOTIFICATION_NAMESPACE: &str = "org.freedesktop.Notifications";
const NOTIFICATION_OBJECTPATH: &str = "/org/freedesktop/Notifications";
const NOTIFICATION_SOURCE: &str = "mpris-notifier";

pub(crate) struct Notifier {
    previous_track_id: Option<String>,
    previous_status: Option<PlayerStatus>,
    configuration: Configuration,
}

// HACK - This is used to get Rustbus to marshal a nested Dict has <String,
// Variant> (rather than as <String, String>).
#[derive(Marshal, Unmarshal, Signature, PartialEq, Eq, Debug)]
enum HintVariant {
    Hint(String),
}

impl Notifier {
    pub(crate) fn new(configuration: &Configuration) -> Self {
        Self {
            previous_track_id: None,
            previous_status: None,
            configuration: configuration.clone(),
        }
    }

    pub(crate) fn handle_signal(
        &mut self,
        signal: MarshalledMessage,
        dbus: &mut DBusConnection,
    ) -> Result<(), DBusError> {
        if let Ok(metadata) = PlayerMetadata::try_from(signal) {
            let previous_track_id = self.previous_track_id.replace(metadata.track_id.clone());
            let previous_status = self.previous_status.replace(metadata.status.clone());

            // Don't notify if the player is pausing
            if metadata.status != PlayerStatus::Playing {
                return Ok(());
            }

            // Don't notify for the same track twice, unless we're resuming play
            if previous_track_id.is_some_and(|id| *id == metadata.track_id)
                || previous_status.is_some_and(|status| *status != PlayerStatus::Playing)
            {
                return Ok(());
            }

            return self.send_notification(&metadata, dbus);
        }

        // Signal we don't care about is ignored
        Ok(())
    }

    fn send_notification(
        &self,
        metadata: &PlayerMetadata,
        dbus: &mut DBusConnection,
    ) -> Result<(), DBusError> {
        // See: https://github.com/hoodie/notify-rust/blob/main/src/xdg/dbus_rs.rs#L64-L73
        let mut message = MessageBuilder::new()
            .call("Notify")
            .at(NOTIFICATION_NAMESPACE)
            .on(NOTIFICATION_OBJECTPATH)
            .with_interface(NOTIFICATION_NAMESPACE)
            .build();

        let subject = self.format_metadata(&self.configuration.subject_format, metadata);
        let body = self.format_metadata(&self.configuration.body_format, metadata);

        message.body.push_param(NOTIFICATION_SOURCE)?; // appname (TODO)
        message.body.push_param(0_u32)?; // update ID
        message.body.push_param("")?; // icon
        message.body.push_param(subject)?; // summary
        message.body.push_param(body)?; // body
        message.body.push_param(Vec::<String>::new())?; // actions (array of strings)
        let mut hints: HashMap<String, HintVariant> = HashMap::new();
        hints.insert(
            "x-canonical-private-synchronous".to_string(),
            HintVariant::Hint(NOTIFICATION_SOURCE.to_string()),
        );
        message.body.push_param(&hints)?; // hints (dict of a{sv})
        message.body.push_param(-1_i32)?; // timeout

        dbus.send_message(&message)
    }

    // Very permissive parsing algorithm (markup).
    fn format_metadata(&self, fmt: &str, metadata: &PlayerMetadata) -> String {
        FormattedNotification::new(fmt, metadata, &self.configuration.join_string).to_string()
    }
}
