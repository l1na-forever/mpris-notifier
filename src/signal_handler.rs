#[cfg(feature = "album-art")]
use crate::art::ArtFetcher;

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

    previous_track_id: Option<String>,
    previous_status: Option<PlayerStatus>,
}

impl SignalHandler {
    pub fn new(configuration: &Configuration) -> Self {
        Self {
            configuration: configuration.clone(),
            notifier: Notifier::new(configuration),
            art_fetcher: ArtFetcher::new(configuration),
            previous_status: None,
            previous_track_id: None,
        }
    }

    pub fn handle_signal(
        &mut self,
        signal: MarshalledMessage,
        dbus: &mut DBusConnection,
    ) -> Result<(), SignalHandlerError> {
        if let Ok(metadata) = PlayerMetadata::try_from(signal) {
            let previous_track_id = self.previous_track_id.replace(metadata.track_id.clone());
            let previous_status = self.previous_status.replace(metadata.status.clone());

            // Don't notify if the player is pausing
            if metadata.status != PlayerStatus::Playing {
                return Ok(());
            }

            // Don't notify for the same track twice, unless we're resuming play
            if previous_track_id.is_some()
                && (previous_track_id.unwrap() == metadata.track_id
                    || previous_status.unwrap() != PlayerStatus::Playing)
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

            return Ok(self
                .notifier
                .send_notification(&metadata, album_art, dbus)?);
        }

        // Signal we don't care about is ignored
        Ok(())
    }
}
