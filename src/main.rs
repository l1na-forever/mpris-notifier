#![feature(is_some_with)]

mod configuration;
mod dbus;
mod formatter;
mod mpris;
mod notifier;

use crate::configuration::{load_configuration, Configuration, ConfigurationError};
use crate::dbus::{DBusConnection, DBusError};
use crate::mpris::subscribe_mpris;
use crate::notifier::Notifier;
use thiserror::Error;

/// Top-level application errors, meant to be presented to the user.
#[derive(Debug, Error)]
enum AppError {
    #[error("error using session D-Bus")]
    DBus(#[from] DBusError),

    #[error("error loading configuration")]
    Configuration(#[from] ConfigurationError),
}

struct App {
    notifier: Notifier,
}

impl App {
    /// Blocks, acting as the main loop.
    fn event_loop(&mut self) -> Result<(), AppError> {
        let mut dbus = DBusConnection::new()?;
        subscribe_mpris(&mut dbus)?;

        loop {
            match dbus.next_signal() {
                Ok(signal) => {
                    if let Err(err) = self.notifier.handle_signal(signal, &mut dbus) {
                        log::error!("{:?}", err);
                    }
                }
                Err(err) => log::error!("{:?}", err),
            }
        }
    }

    fn new(configuration: &Configuration) -> Result<Self, AppError> {
        Ok(Self {
            notifier: Notifier::new(configuration),
        })
    }
}

fn main() -> Result<(), AppError> {
    simple_logger::init_with_level(log::Level::Warn).unwrap();
    let configuration = load_configuration()?;
    let mut app = App::new(&configuration)?;
    app.event_loop()?;
    Ok(())
}
