#![feature(is_some_with)]

#[cfg(feature = "album-art")]
mod art;

mod configuration;
mod dbus;
mod formatter;
mod mpris;
mod notifier;
mod signal_handler;

use crate::configuration::{load_configuration, Configuration, ConfigurationError};
use crate::dbus::{DBusConnection, DBusError};
use crate::mpris::subscribe_mpris;
use crate::signal_handler::SignalHandler;
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
    signal_handler: SignalHandler,
}

impl App {
    /// Blocks, acting as the main loop.
    fn event_loop(&mut self) -> Result<(), AppError> {
        let mut dbus = DBusConnection::new()?;
        subscribe_mpris(&mut dbus)?;

        loop {
            match dbus.next_signal() {
                Ok(signal) => {
                    if let Err(err) = self.signal_handler.handle_signal(signal, &mut dbus) {
                        log::error!("{:?}", err);
                    }
                }
                Err(err) => log::error!("{:?}", err),
            }
        }
    }

    fn new(configuration: &Configuration) -> Result<Self, AppError> {
        Ok(Self {
            signal_handler: SignalHandler::new(configuration),
        })
    }
}

fn main() -> Result<(), AppError> {
    simple_logger::init_with_level(log::Level::Info).unwrap();
    let configuration = load_configuration()?;
    let mut app = App::new(&configuration)?;
    app.event_loop()?;
    Ok(())
}
