use crate::dbus::{DBusConnection, DBusError};
use rustbus::message_builder::MarshalledMessage;
use rustbus::wire::unmarshal::traits::Variant;
use std::collections::HashMap;
use std::str::FromStr;

const MPRIS_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";

#[derive(Debug, Clone)]
pub(crate) struct PlayerMetadata {
    pub(crate) status: PlayerStatus,
    pub(crate) track_id: String,

    pub(crate) album: Option<String>,
    pub(crate) album_artists: Option<Vec<String>>,
    pub(crate) art_url: Option<String>,
    pub(crate) artists: Option<Vec<String>>,
    pub(crate) title: Option<String>,
    pub(crate) track_number: Option<u32>,
    pub(crate) track_url: Option<String>,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum PlayerStatus {
    Playing,
    Paused,
    Stopped,
}

impl FromStr for PlayerStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Playing" => Ok(PlayerStatus::Playing),
            "Paused" => Ok(PlayerStatus::Paused),
            "Stopped" => Ok(PlayerStatus::Stopped),
            _ => Err(()),
        }
    }
}

impl TryFrom<MarshalledMessage> for PlayerMetadata {
    type Error = DBusError;

    fn try_from(message: MarshalledMessage) -> Result<Self, Self::Error> {
        let mut parser = message.body.parser();
        let interface_name: &str = parser.get()?;
        if interface_name != MPRIS_INTERFACE {
            return Err(DBusError::Invalid(format!(
                "wrong interface type '{}'",
                interface_name
            )));
        }

        let outer: HashMap<String, Variant> = parser.get()?;
        let inner: HashMap<String, Variant> = outer
            .get("Metadata")
            .ok_or_else(|| DBusError::Invalid("Missing Metadata envelope".to_string()))?
            .get()?;

        Ok(Self {
            status: PlayerStatus::from_str(
                outer
                    .get("PlaybackStatus")
                    .ok_or_else(|| {
                        DBusError::Invalid("Missing PlaybackStatus envelope".to_string())
                    })?
                    .get()?,
            )
            .unwrap_or(PlayerStatus::Stopped),
            track_id: inner["mpris:trackid"].get()?,

            album: inner.get("xesam:album").and_then(|x| x.get().ok()),
            album_artists: inner.get("xesam:albumArtist").and_then(|x| x.get().ok()),
            art_url: inner.get("mpris:artUrl").and_then(|x| x.get().ok()),
            artists: inner.get("xesam:artist").and_then(|x| x.get().ok()),
            title: inner.get("xesam:title").and_then(|x| x.get().ok()),
            track_number: inner.get("xesam:trackNumber").and_then(|x| x.get().ok()),
            track_url: inner.get("xesam:url").and_then(|x| x.get().ok()),
        })
    }
}

// Convenience method to subscribe a DBusConnection to MPRIS player property
// change events (e.g., track changes).
pub(crate) fn subscribe_mpris(dbus: &mut DBusConnection) -> Result<(), DBusError> {
    dbus.subscribe(
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
        "/org/mpris/MediaPlayer2",
    )
}
