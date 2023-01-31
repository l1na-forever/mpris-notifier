use crate::dbus::{DBusConnection, DBusError};
use rustbus::message_builder::MarshalledMessage;
use rustbus::wire::unmarshal::traits::Variant;
use std::collections::HashMap;
use std::str::FromStr;

const MPRIS_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";
const MPRIS_SIGNAL_INTERFACE: &str = "org.freedesktop.DBus.Properties";
const MPRIS_SIGNAL_MEMBER: &str = "PropertiesChanged";
const MPRIS_SIGNAL_OBJECT: &str = "/org/mpris/MediaPlayer2";

#[derive(Debug, Clone)]
pub struct MprisPropertiesChange {
    pub status: Option<PlayerStatus>,
    pub metadata: Option<PlayerMetadata>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PlayerMetadata {
    pub track_id: Option<String>,
    pub album: Option<String>,
    pub album_artists: Option<Vec<String>>,
    pub art_url: Option<String>,
    pub artists: Option<Vec<String>>,
    pub title: Option<String>,
    pub track_number: Option<u32>,
    pub track_url: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PlayerStatus {
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

impl TryFrom<MarshalledMessage> for MprisPropertiesChange {
    type Error = DBusError;

    fn try_from(message: MarshalledMessage) -> Result<Self, Self::Error> {
        let mut parser = message.body.parser();
        let interface_name: &str = parser.get()?;
        if interface_name != MPRIS_INTERFACE {
            return Err(DBusError::Invalid(format!(
                "wrong interface type '{interface_name}'"
            )));
        }

        // HashMap<String, Variant>
        let outer: HashMap<String, Variant> = parser.get()?;
        let metadata_map: Option<HashMap<String, Variant>> =
            outer.get("Metadata").and_then(|m| m.get().ok());
        let status: Option<PlayerStatus> = outer
            .get("PlaybackStatus")
            .and_then(|s| s.get().ok())
            .and_then(|s| PlayerStatus::from_str(s).ok());
        let metadata: Option<PlayerMetadata> = metadata_map.map(|m| metadata_from_map(&m));

        Ok(Self { status, metadata })
    }
}

fn metadata_from_map(inner: &HashMap<String, Variant>) -> PlayerMetadata {
    PlayerMetadata {
        track_id: inner.get("mpris:trackid").and_then(|x| x.get().ok()),
        album: inner.get("xesam:album").and_then(|x| x.get().ok()),
        album_artists: inner.get("xesam:albumArtist").and_then(|x| x.get().ok()),
        art_url: inner.get("mpris:artUrl").and_then(|x| x.get().ok()),
        artists: inner.get("xesam:artist").and_then(|x| x.get().ok()),
        title: inner.get("xesam:title").and_then(|x| x.get().ok()),
        track_number: inner.get("xesam:trackNumber").and_then(|x| x.get().ok()),
        track_url: inner.get("xesam:url").and_then(|x| x.get().ok()),
    }
}

// Convenience method to subscribe a DBusConnection to MPRIS player property
// change events (e.g., track changes).
pub fn subscribe_mpris(dbus: &mut DBusConnection) -> Result<(), DBusError> {
    dbus.subscribe(
        MPRIS_SIGNAL_INTERFACE,
        MPRIS_SIGNAL_MEMBER,
        MPRIS_SIGNAL_OBJECT,
    )
}
