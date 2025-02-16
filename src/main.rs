#[cfg(feature = "album-art")]
mod art;

mod configuration;
mod dbus;
mod formatter;
mod message_handler;
mod mpris;
mod notifier;

use crate::configuration::{load_configuration, Configuration, ConfigurationError};
use crate::dbus::{DBusConnection, DBusError};
use crate::message_handler::MessageHandler;
use crate::mpris::subscribe_mpris;
use core::panic;
use crossbeam_channel::{Receiver, Sender};
use rustbus::message_builder::MarshalledMessage;
use std::time::Duration;
use std::time::Instant;
use thiserror::Error;

// During the debounce period, the message handler waits for additional MPRIS
// messages before firing. Some MPRIS clients fire a bunch of notifications in a
// row that need to be processed as a group, such as Firefox.
//
// This value was determined from a bunch of not-super-scientific tests on my
// system, measuring the deltas between the multiple MPRIS notifications that
// Firefox fires:
//   [mpris_notifier] delta: 6.68µs
//   [mpris_notifier] delta: 5.568159ms
//   [mpris_notifier] delta: 5.60077ms
//   [mpris_notifier] delta: 27.683741ms
//   [mpris_notifier] done after timeout: 100.074542ms
const DEBOUNCE_PERIOD: Duration = Duration::from_millis(100);

/// Top-level application errors, meant to be presented to the user.
#[derive(Debug, Error)]
enum AppError {
    #[error("error using session D-Bus")]
    DBus(#[from] DBusError),

    #[error("error loading configuration")]
    Configuration(#[from] ConfigurationError),

    #[error("{0}")]
    CrossbeamSendError(#[from] Box<crossbeam_channel::SendError<MarshalledMessage>>),

    #[error("{0}")]
    CrossbeamRecvError(#[from] crossbeam_channel::RecvError),

    #[error("{0}")]
    CrossbeamRecvTimeoutError(#[from] crossbeam_channel::RecvTimeoutError),
}

// DBus polling thread, generating messages for consumption on the main
// thread.
fn dbus_thread(dbus_tx: Sender<MarshalledMessage>) -> Result<(), AppError> {
    let mut dbus = DBusConnection::new()?;
    subscribe_mpris(&mut dbus)?;

    loop {
        match dbus.next_message() {
            Ok(message) => {
                dbus_tx.send(message).map_err(Box::new)?;
            }
            Err(DBusError::Connection(rustbus::connection::Error::TimedOut)) => {}
            Err(err) => {
                log::error!("error polling D-Bus: {:?}", err)
            }
        }
    }
}

/// Handles messages generated from DBus and firing notifications.
fn message_thread(
    dbus_rx: Receiver<MarshalledMessage>,
    mut message_handler: MessageHandler,
) -> Result<(), AppError> {
    let mut dbus = DBusConnection::new()?;

    loop {
        // Wait indefinitely for a DBus message.
        let mut message = dbus_rx.recv()?;

        // Accept DBus signals until the debounce period completes. Because
        // handle_signal downloads album art synchronously, this will block
        // for max(debounce, album art deadline).
        let start = Instant::now();
        loop {
            if let Err(err) = message_handler.process_message(message) {
                log::error!("error processing message: {:?}", err);
            }

            let delta = Instant::now() - start;
            if delta > DEBOUNCE_PERIOD {
                break;
            }

            match dbus_rx.recv_timeout(DEBOUNCE_PERIOD - delta) {
                Ok(new_message) => message = new_message,
                _ => {
                    break;
                }
            }
        }

        // Handle the pending notification.
        if let Err(err) = message_handler.fire_pending(&mut dbus) {
            log::error!("error sending notification: {:?}", err);
        }
    }
}

fn main() -> Result<(), AppError> {
    simple_logger::init_with_level(log::Level::Info).unwrap();
    let configuration = load_configuration()?;
    let message_handler = MessageHandler::new(&configuration);
    let (dbus_tx, dbus_rx) = crossbeam_channel::unbounded();

    std::thread::spawn(|| {
        match dbus_thread(dbus_tx) {
            Ok(_) => log::error!("DBus thread exited early"),
            Err(err) => log::error!("error polling DBus: {}", err),
        }
        panic!();
    });
    message_thread(dbus_rx, message_handler)?;
    Ok(())
}
