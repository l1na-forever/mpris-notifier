use crate::configuration::Configuration;
use crate::notifier::NotificationImage;
use image::io::Reader as ImageReader;
use std::io::{Cursor, Read};
use std::time::Duration;
use thiserror::Error;

const ART_SIZE_LIMIT: usize = 5_000_000; // ~5MB download size limit

#[derive(Debug, Error)]
pub enum ArtFetcherError {
    #[error("error fetching URL")]
    Fetch(#[from] ureq::Error),

    #[error("error writing tempfile")]
    Write(#[from] std::io::Error),

    #[error("error decoding image")]
    Decode(#[from] image::ImageError),

    #[error("invalid response")]
    Invalid(),
}

pub struct ArtFetcher {
    timeout: Duration,
}

impl ArtFetcher {
    pub fn new(configuration: &Configuration) -> Self {
        Self {
            timeout: Duration::from_millis(configuration.album_art_deadline.into()),
        }
    }

    pub fn get_album_art(&self, url: &str) -> Result<NotificationImage, ArtFetcherError> {
        let body = self.fetch_url(url)?;
        let image = ImageReader::new(Cursor::new(body))
            .with_guessed_format()?
            .decode()?;
        Ok(NotificationImage::from(image))
    }

    fn fetch_url(&self, url: &str) -> Result<Vec<u8>, ArtFetcherError> {
        let response = ureq::get(url).timeout(self.timeout).call()?;

        let len: usize = response
            .header("content-length")
            .ok_or(ArtFetcherError::Invalid())?
            .parse()
            .map_err(|_| ArtFetcherError::Invalid())?;
        let mut bytes: Vec<u8> = Vec::with_capacity(std::cmp::max(len, ART_SIZE_LIMIT));
        response
            .into_reader()
            .take(ART_SIZE_LIMIT.try_into().unwrap())
            .read_to_end(&mut bytes)?;

        Ok(bytes)
    }
}
