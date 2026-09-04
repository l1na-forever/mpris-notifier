#[cfg(feature = "album-art")]
use crate::art::ArtFetcher;

use crate::dbus::DBusError;
use crate::mpris::MprisPropertiesChange;
use crate::mpris::PlayerMetadata;
use crate::mpris::PlayerStatus;
use crate::notifier::Notification;
use crate::{configuration::Configuration, dbus::DBusConnection, notifier::Notifier};
use rustbus::message_builder::MarshalledMessage;
use std::collections::HashMap;
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MessageHandlerError {
    #[error("error handling D-Bus message")]
    DBus(#[from] DBusError),
}

pub struct MessageHandler {
    configuration: Configuration,
    notifier: Notifier,
    art_fetcher: ArtFetcher,

    // Map from <D-Bus Sender> -> <Last Received Metadata>
    metadata: HashMap<String, PlayerMetadata>,

    // Map from <D-Bus Sender> -> <Last Received Status>
    status: HashMap<String, PlayerStatus>,

    // Notification that will be sent after [DEBOUNCE_PERIOD] passes.
    pending_notification: Option<Notification>,

    // Map from unique D-Bus name (":1.N") -> well-known MPRIS name
    // ("org.mpris.MediaPlayer2.<player>"). Used for allowlist matching.
    name_map: HashMap<String, String>,
}

impl MessageHandler {
    pub fn new(configuration: &Configuration) -> Self {
        Self {
            configuration: configuration.clone(),
            notifier: Notifier::new(configuration),
            art_fetcher: ArtFetcher::new(configuration),
            metadata: HashMap::new(),
            pending_notification: None,
            status: HashMap::new(),
            name_map: HashMap::new(),
        }
    }

    pub fn load_initial_players(&mut self, map: HashMap<String, String>) {
        for (unique, well_known) in map {
            self.name_map.insert(unique, well_known);
        }
    }

    // Must be called regularly from the main loop. Used to fire notifications
    // on a timer.
    pub fn fire_pending(&mut self, dbus: &mut DBusConnection) -> Result<(), MessageHandlerError> {
        if let Some(pending) = self.pending_notification.take() {
            self.notifier.send_notification(pending, dbus)?;
            self.fire_commands();
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

    // Called from the main loop for every received message. Handles
    // NameOwnerChanged signals to maintain the unique→well-known name map,
    // and sets the pending notification for MPRIS property changes.
    // Does not emit the notification; use [fire_pending] to send it.
    pub fn process_message(
        &mut self,
        message: MarshalledMessage,
    ) -> Result<(), MessageHandlerError> {
        // Track NameOwnerChanged to build unique-name -> well-known-name map.
        if message
            .dynheader
            .member
            .as_deref()
            .is_some_and(|m| m == "NameOwnerChanged")
        {
            self.handle_name_owner_changed(&message);
            return Ok(());
        }

        let sender = message
            .dynheader
            .sender
            .as_ref()
            .ok_or_else(|| DBusError::Invalid("Missing sender header".to_string()))?
            .clone();

        let change = MprisPropertiesChange::try_from(message).ok();

        // Signals we don't care about are ignored
        if change.is_none() {
            return Ok(());
        }
        let change = change.unwrap();

        // Apply allowlist if configured: check if this sender's well-known
        // name contains any of the allowlisted player name substrings.
        if let Some(allowlist) = &self.configuration.player_allowlist {
            let well_known = self.name_map.get(&sender).map(String::as_str).unwrap_or("unknown");
            let allowed = allowlist
                .iter()
                .any(|entry| well_known.contains(entry.as_str()));
            if !allowed {
                log::debug!(
                    "Ignoring signal from '{}' (well-known: '{}'), not in allowlist",
                    sender,
                    well_known
                );
                return Ok(());
            }
        }

        // Handle metadata property changes.
        //
        // Incoming metadata property changes are cached per each sender,
        // where the most recently received metadata is cached in its
        // entirety.
        //
        // A property change always queues up a notification to be sent.
        let mut metadata: Option<&PlayerMetadata> = self.metadata.get(&sender);
        if let Some(new_metadata) = change.metadata {
            let old_metadata = self.metadata.get(&sender);

            // Check if metadata has actually changed
            let metadata_changed =
                old_metadata.is_none() || old_metadata.is_some_and(|old| old != &new_metadata);

            if metadata_changed {
                self.metadata
                    .insert(sender.to_string(), new_metadata.clone());
                metadata = self.metadata.get(&sender);

                // Wipe out player status whenever the track metadata changes.
                // Player status is used to ensure that Playing -> Playing status
                // changes don't generate spurious notifications.
                self.status.remove(&sender);

                // If our current notification is from the same sender, update it.
                // Otherwise, wipe out whatever was being built and start
                // hydrating a new Notification.
                if let Some(pending) = self.pending_notification.as_mut() {
                    if pending.sender() == sender {
                        pending.update(&new_metadata, None);
                    }
                } else {
                    self.pending_notification =
                        Some(Notification::new(&sender, &new_metadata, None));
                }
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

    // Handles a NameOwnerChanged signal from org.freedesktop.DBus to maintain
    // the unique-name -> well-known-name map used for allowlist matching.
    // The signal body is (name: &str, old_owner: &str, new_owner: &str):
    //   - new_owner non-empty: a service acquired the name
    //   - new_owner empty: a service released the name
    fn handle_name_owner_changed(&mut self, message: &MarshalledMessage) {
        let mut parser = message.body.parser();
        let name = match parser.get::<&str>() {
            Ok(s) => s,
            Err(_) => return,
        };
        let _old_owner = match parser.get::<&str>() {
            Ok(s) => s,
            Err(_) => return,
        };
        let new_owner = match parser.get::<&str>() {
            Ok(s) => s,
            Err(_) => return,
        };

        // NameOwnerChanged fires for every D-Bus service, not just MPRIS players.
        // Skip non-MPRIS names to avoid accumulating irrelevant entries in name_map.
        if !name.starts_with("org.mpris.MediaPlayer2.") {
            return;
        }

        if new_owner.is_empty() {
            // Player unregistered: remove from map by value.
            self.name_map.retain(|_, v| v != name);
            log::debug!("MPRIS player unregistered: {}", name);
        } else {
            // Player registered: map unique name -> well-known name.
            self.name_map.insert(new_owner.to_string(), name.to_string());
            log::debug!("MPRIS player registered: {} -> {}", new_owner, name);
        }
    }
}
