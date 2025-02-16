#[cfg(feature = "album-art")]
use crate::art::ArtFetcher;

use crate::mpris::MprisPropertiesChange;
use crate::mpris::PlayerMetadata;
use crate::mpris::PlayerStatus;
use crate::notifier::Notification;
use crate::DBusError;
use crate::{configuration::Configuration, dbus::DBusConnection, notifier::Notifier};
use rustbus::message_builder::MarshalledMessage;
use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;
use thiserror::Error;

// After receiving a track changed signal, the notification is held for this
// period of time before being sent, to allow for more changes to be sent.
// Some clients send multiple `PropertiesChanged` signals adding additional
// metadata fields.
const NOTIFICATION_DEBOUNCE: Duration = Duration::from_millis(10);

#[derive(Debug, Error)]
pub enum SignalHandlerError {
    #[error("error handling D-Bus signal")]
    DBus(#[from] DBusError),
}

pub struct SignalHandler {
    configuration: Configuration,
    notifier: Notifier,
    art_fetcher: ArtFetcher,

    // Map from <D-Bus Sender> -> <Last Received Metadata>
    metadata: HashMap<String, PlayerMetadata>,

    // Map from <D-Bus Sender> -> <Last Received Status>
    status: HashMap<String, PlayerStatus>,

    // Notification that will be sent after [NOTIFICATION_DELAY] passes.
    pending_notification: Option<Notification>,
}

impl SignalHandler {
    pub fn new(configuration: &Configuration) -> Self {
        Self {
            configuration: configuration.clone(),
            notifier: Notifier::new(configuration),
            art_fetcher: ArtFetcher::new(configuration),
            metadata: HashMap::new(),
            pending_notification: None,
            status: HashMap::new(),
        }
    }

    // Must be called regularly from the main loop. Used to fire notifications
    // on a timer.
    //
    // TODO - rename to fire_pending, move debounce logic out
    pub fn handle_pending(&mut self, dbus: &mut DBusConnection) -> Result<(), SignalHandlerError> {
        if let Some(pending) = &self.pending_notification {
            let delta = Instant::now() - pending.last_touched();

            if delta > NOTIFICATION_DEBOUNCE {
                self.notifier
                    .send_notification(self.pending_notification.take().unwrap(), dbus)?;
                self.fire_commands();
            }
        }

        Ok(())
    }

    // Instantiates Command instances based on the configured commands.
    fn generate_commands(&self) -> Vec<Command> {
        let config_commands = self.configuration.commands.clone();
        if config_commands.is_none() {
            return Vec::new();
        }

        config_commands
            .unwrap()
            .iter()
            .filter_map(|command_args| match command_args.len() {
                0 => None,
                1 => Some(Command::new(command_args[0].as_str())),
                2.. => {
                    let mut cmd = Command::new(command_args[0].as_str());
                    cmd.args(&command_args[1..command_args.len()]);
                    Some(cmd)
                }
            })
            .collect()
    }

    // Fires commands after a notification was sent.
    fn fire_commands(&self) {
        let mut commands = self.generate_commands();
        for command in commands.iter_mut() {
            match command.output() {
                Ok(_) => (),
                Err(err) => {
                    log::warn!("Command failed: {}", err);
                }
            }
        }
    }

    // Called from the main loop for every received signal. Sets the pending
    // notification, but does not emit the notification; use [handle_pending]
    // to send the notification.
    pub fn handle_signal(&mut self, signal: MarshalledMessage) -> Result<(), SignalHandlerError> {
        let sender = signal
            .dynheader
            .sender
            .as_ref()
            .ok_or_else(|| DBusError::Invalid("Missing sender header".to_string()))?
            .clone();
        let change = MprisPropertiesChange::try_from(signal).ok();

        // Signals we don't care about are ignored
        if change.is_none() {
            return Ok(());
        }
        let change = change.unwrap();

        // Handle metadata property changes.
        //
        // Incoming metadata property changes are cached per each sender,
        // where the most recently received metadata is cached in its
        // entirety.
        //
        // A property change always queues up a notification to be sent.
        let mut metadata: Option<&PlayerMetadata> = self.metadata.get(&sender);
        if let Some(new_metadata) = change.metadata {
            self.metadata
                .insert(sender.to_string(), new_metadata.clone());
            metadata = self.metadata.get(&sender);

            // Wipe out player status whenever we have incoming track metadata.
            // Player status is used to ensure that Playing -> Playing status
            // changes don't generate spurious notifications.
            self.status.remove(&sender);

            // If our current notification is from the same sender, update it.
            // Otherwise, wipe out whatever was being built and start
            // hydrating a new Notification.
            if let Some(pending) = self.pending_notification.as_mut()
                && pending.sender() == sender
            {
                pending.update(&new_metadata, None);
            } else {
                self.pending_notification = Some(Notification::new(&sender, &new_metadata, None));
            }
        }

        // If we haven't gotten metadata yet, we can't notify
        if metadata.is_none() {
            return Ok(());
        }
        let metadata = metadata.unwrap();

        // Handle playback status.
        //
        // When the 'Playing' signal is sent, queue that sender's track
        // for notification (either they're resuming play, or changing
        // tracks).
        if let Some(status) = change.status {
            let last_status = self.status.insert(sender.clone(), status.clone());

            if status == PlayerStatus::Playing {
                // We only want to generate a notification for a "Playing" status
                // change when we weren't already in "Playing".
                if last_status.is_none() || last_status.is_some_and(|l| l != PlayerStatus::Playing)
                {
                    self.pending_notification = Some(Notification::new(&sender, metadata, None));
                }
            } else {
                self.pending_notification = None;
            }
        }

        //  We can't notify if the pending notification is still empty
        if self.pending_notification.as_mut().is_none() {
            return Ok(());
        }
        let pending = self.pending_notification.as_mut().unwrap();

        // Fetch album art to a temporary buffer in the pending notification,
        // if the feature is enabled.
        #[cfg(feature = "album-art")]
        if metadata.art_url.is_some() && self.configuration.enable_album_art {
            let result = self
                .art_fetcher
                .get_album_art(metadata.art_url.as_ref().unwrap());
            match result {
                Ok(data) => {
                    pending.update(metadata, Some(data));
                }
                Err(err) => {
                    log::warn!("Error fetching album art for {:#?}: {}", &metadata, err);
                }
            }
        }

        Ok(())
    }
}
