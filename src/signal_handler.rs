#[cfg(feature = "album-art")]
use crate::art::ArtFetcher;

use crate::mpris::MprisPropertiesChange;
use crate::mpris::PlayerMetadata;
use crate::mpris::PlayerStatus;
use crate::notifier::NotificationImage;
use crate::DBusError;
use crate::{configuration::Configuration, dbus::DBusConnection, notifier::Notifier};
use rustbus::message_builder::MarshalledMessage;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignalHandlerError {
    #[error("error handling D-Bus signal")]
    DBus(#[from] DBusError),
}

pub struct SignalHandler {
    configuration: Configuration,
    notifier: Notifier,
    art_fetcher: ArtFetcher,

    status: HashMap<String, PlayerStatus>,
    metadata: HashMap<String, PlayerMetadata>,
}

impl SignalHandler {
    pub fn new(configuration: &Configuration) -> Self {
        Self {
            configuration: configuration.clone(),
            notifier: Notifier::new(configuration),
            art_fetcher: ArtFetcher::new(configuration),
            status: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn handle_signal(
        &mut self,
        signal: MarshalledMessage,
        dbus: &mut DBusConnection,
    ) -> Result<(), SignalHandlerError> {
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

        let mut status: Option<&PlayerStatus> = self.status.get(&sender);
        let mut metadata: Option<&PlayerMetadata> = self.metadata.get(&sender);
        let mut previous_status: Option<PlayerStatus> = None;
        let mut previous_metadata: Option<PlayerMetadata> = None;

        if change.status.is_some() {
            previous_status = self
                .status
                .insert(sender.to_string(), change.status.unwrap());
            status = self.status.get(&sender);
        }
        if change.metadata.is_some() {
            previous_metadata = self
                .metadata
                .insert(sender.to_string(), change.metadata.unwrap());
            metadata = self.metadata.get(&sender);
        }

        // If we haven't gotten metadata/status yet, we can't notify
        if metadata.is_none() || status.is_none() {
            return Ok(());
        }
        let metadata = metadata.unwrap();
        let status = status.unwrap();

        log::info!(
            "status is {:?}, metadata is {:#?}, previous_metadata is {:#?}",
            &status,
            &metadata,
            &previous_metadata
        );

        if *status != PlayerStatus::Playing {
            return Ok(());
        }

        // Don't notify if a notification for this track has already fired, unless we're resuming after pause.
        if (previous_status.is_some() && previous_status.unwrap() != PlayerStatus::Paused)
            && previous_metadata.is_some()
            && previous_metadata.unwrap() == *metadata
        {
            return Ok(());
        }

        // Fetch album art to a temporary buffer, if the feature is enabled.
        let mut album_art: Option<NotificationImage> = None;

        #[cfg(feature = "album-art")]
        if metadata.art_url.is_some() && self.configuration.enable_album_art {
            let result = self
                .art_fetcher
                .get_album_art(metadata.art_url.as_ref().unwrap());
            match result {
                Ok(data) => album_art = Some(data),
                Err(err) => {
                    log::warn!("Error fetching album art for {:#?}: {}", &metadata, err);
                }
            }
        }

        Ok(self.notifier.send_notification(metadata, album_art, dbus)?)
    }
}
