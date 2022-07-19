use crate::mpris::PlayerMetadata;
use lazy_static::lazy_static;
use regex::{Regex, Replacer};
use std::fmt;

lazy_static! {
    static ref FORMAT_REGEX: Regex = Regex::new(r"(\{[^\}]+\})").unwrap();
    static ref EMPTY_STR: String = String::from("");
}

pub struct FormattedNotification<'a> {
    fmt: &'a str,
    metadata: &'a PlayerMetadata,
    join_str: &'a str,
}

impl<'a> fmt::Display for FormattedNotification<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", FORMAT_REGEX.replace_all(self.fmt, self))
    }
}

impl<'a> Replacer for &FormattedNotification<'_> {
    fn replace_append(&mut self, caps: &regex::Captures<'_>, dst: &mut String) {
        let cap = caps.get(0).unwrap().as_str();
        let md = &self.metadata;
        match cap {
            "{album}" => dst.push_str(unwrap_str_field(&md.album)),
            "{album_artists}" => dst.push_str(&unwrap_vec_field(&md.album_artists, self.join_str)),
            "{album_artist}" => dst.push_str(&unwrap_vec_field(&md.album_artists, self.join_str)),
            "{artists}" => dst.push_str(&unwrap_vec_field(&md.artists, self.join_str)),
            "{artist}" => dst.push_str(&unwrap_vec_field(&md.artists, self.join_str)),
            "{title}" => dst.push_str(unwrap_str_field(&md.title)),
            "{track}" => dst.push_str(unwrap_str_field(&md.title)),
            "{track_number}" => dst.push_str(&self.metadata.track_number.unwrap_or(1).to_string()),
            _ => dst.push_str(cap), // if we don't recognize the token, leave it as-is
        }
    }
}

fn unwrap_str_field(field: &Option<String>) -> &str {
    field.as_ref().unwrap_or(&EMPTY_STR)
}

// An owned String is returned, because joining the strings necessitates a new
// allocation.
fn unwrap_vec_field(field: &Option<Vec<String>>, join_str: &str) -> String {
    if let Some(entries) = field {
        entries.join(join_str)
    } else {
        "".to_string()
    }
}

impl<'a> FormattedNotification<'a> {
    pub fn new(fmt: &'a str, metadata: &'a PlayerMetadata, join_str: &'a str) -> Self {
        Self {
            fmt,
            metadata,
            join_str,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FormattedNotification;
    use crate::mpris::{PlayerMetadata, PlayerStatus};

    #[test]
    fn test_formatted_notification() {
        let fmt = "{album} {album_artists} {album_artist}
                   {artists} {artist} {title} {track}
                   {track_number} {nop} nop 👻";
        let exp = "vivisect blackwinterwells * 8485 blackwinterwells * 8485
                   blackwinterwells * 8485 blackwinterwells * 8485 vivisect vivisect
                   1 {nop} nop 👻";
        let metadata = PlayerMetadata {
            status: PlayerStatus::Playing,
            track_id: "track-id".to_string(),
            album: Some("vivisect".to_string()),
            album_artists: Some(vec!["blackwinterwells".to_string(), "8485".to_string()]),
            art_url: Some(
                "https://i.scdn.co/image/ab67616d0000b2734cb12eb0f39785eba7f73c22".to_string(),
            ),
            artists: Some(vec!["blackwinterwells".to_string(), "8485".to_string()]),
            title: Some("vivisect".to_string()),
            track_number: Some(1),
            track_url: Some("https://open.spotify.com/track/4C4YkH503GMmFv4gZ5cuXv".to_string()),
        };
        let join_str = " * ";
        let notification = FormattedNotification::new(fmt, &metadata, join_str);
        let result = notification.to_string();

        assert_eq!(exp, result);
    }

    #[test]
    fn test_formatted_notification_empty() {
        let fmt = "{album} {album_artists} {album_artist} {artists} {artist} {title} {track} {track_number} {nop} nop";
        let exp = "       1 {nop} nop";
        let metadata = PlayerMetadata {
            status: PlayerStatus::Playing,
            track_id: "track-id".to_string(),
            album: None,
            album_artists: None,
            art_url: None,
            artists: None,
            title: None,
            track_number: None,
            track_url: None,
        };
        let join_str = " * ";
        let notification = FormattedNotification::new(fmt, &metadata, join_str);
        let result = notification.to_string();

        assert_eq!(exp, result);
    }
}
