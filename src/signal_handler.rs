#[cfg(feature = "album-art")]
use crate::art::ArtFetcher;

use crate::mpris::MprisPropertiesChange;
use crate::mpris::PlayerMetadata;
use crate::mpris::PlayerStatus;
use crate::notifier::NotificationImage;
use crate::DBusError;
use crate::{configuration::Configuration, dbus::DBusConnection, notifier::Notifier};
use rustbus::message_builder::MarshalledMessage;
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

    status: Option<PlayerStatus>,
    metadata: Option<PlayerMetadata>,
}

impl SignalHandler {
    pub fn new(configuration: &Configuration) -> Self {
        Self {
            configuration: configuration.clone(),
            notifier: Notifier::new(configuration),
            art_fetcher: ArtFetcher::new(configuration),
            status: None,
            metadata: None,
        }
    }

    pub fn handle_signal(
        &mut self,
        signal: MarshalledMessage,
        dbus: &mut DBusConnection,
    ) -> Result<(), SignalHandlerError> {
        let change = MprisPropertiesChange::try_from(signal).ok();
        // Signals we don't care about are ignored
        if change.is_none() {
            return Ok(());
        }
        let change = change.unwrap();

        let mut status: Option<&PlayerStatus> = self.status.as_ref();
        let mut metadata: Option<&PlayerMetadata> = self.metadata.as_ref();
        let mut previous_metadata: Option<PlayerMetadata> = None;

        if change.status.is_some() {
            self.status.replace(change.status.unwrap());
            status = self.status.as_ref();
        }
        if change.metadata.is_some() {
            previous_metadata = self.metadata.replace(change.metadata.unwrap());
            metadata = self.metadata.as_ref();
        }

        // If we haven't gotten metadata yet, we can't notify
        if metadata.is_none() {
            return Ok(());
        }
        let metadata = metadata.unwrap();

        // Don't notify if the player is pausing on the same track. Some
        // players (such as Firefox, in testing) won't necessarily fire a
        // PlayerStatus propertieschanged event for cases like page
        // reloads, so we're permissive on what status can be set to.
        if (status.is_some() && *status.unwrap() != PlayerStatus::Playing)
            && (previous_metadata.is_some() && previous_metadata.unwrap() == *metadata)
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
