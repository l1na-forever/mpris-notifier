use rustbus::message_builder::MarshalledMessage;
use rustbus::DuplexConn;
use std::time::Duration;
use thiserror::Error;

const POLLING_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Error)]
pub enum DBusError {
    #[error("D-Bus connection error")]
    Connection(#[from] rustbus::connection::Error),

    #[error("Unexpected D-Bus message format")]
    Invalid(String),

    #[error("D-Bus error message received")]
    Generic(String),

    #[error("error unmarshalling D-Bus message")]
    Unmarshal(#[from] rustbus::wire::errors::UnmarshalError),

    #[error("error marshalling D-Bus message")]
    Marshal(#[from] rustbus::wire::errors::MarshalError),
}

pub struct DBusConnection {
    connection: DuplexConn,
}

impl DBusConnection {
    pub fn new() -> Result<Self, DBusError> {
        let (connection, _) = Self::connect()?;
        Ok(Self { connection })
    }

    /// Blocks, awaiting the next signal from D-Bus, which is processed and
    /// returned. No-op messages (messages from which no useful result is
    /// derived) are silently acknowledged, and `next_message` will continue
    /// to block until a message that yields a result is received, or the
    /// polling timeout is reached.
    pub fn next_signal(&mut self) -> Result<Option<MarshalledMessage>, DBusError> {
        use rustbus::{connection::Timeout, MessageType};

        loop {
            let message = self
                .connection
                .recv
                .get_next_message(Timeout::Duration(POLLING_TIMEOUT))?;
            match message.typ {
                MessageType::Signal => return Ok(Some(message)),
                MessageType::Error => {
                    let body = self.message_body_string(&message)?;
                    return Err(DBusError::Generic(body.to_string()));
                }
                MessageType::Invalid => {
                    let body = self.message_body_string(&message)?.to_string();
                    return Err(DBusError::Invalid(body));
                }
                _ => {}
            }
        }
    }

    pub fn send_message(&mut self, message: &MarshalledMessage) -> Result<(), DBusError> {
        self.connection
            .send
            .send_message_write_all(message)
            .map(|_| ())
            .map_err(DBusError::Connection)
    }

    fn message_body_string<'a>(
        &self,
        message: &'a MarshalledMessage,
    ) -> Result<&'a str, DBusError> {
        Ok(message.body.parser().get::<&str>()?)
    }

    fn connect() -> Result<(DuplexConn, String), DBusError> {
        use rustbus::{connection::Timeout, get_session_bus_path};

        let session_path = get_session_bus_path()?;
        let mut connection = DuplexConn::connect_to_bus(session_path, true)?;
        let connection_id = connection.send_hello(Timeout::Infinite)?;

        Ok((connection, connection_id))
    }

    pub fn subscribe(
        &mut self,
        interface: &str,
        member: &str,
        path: &str,
    ) -> Result<(), DBusError> {
        use rustbus::standard_messages::add_match;
        let match_str = format!(
            "interface='{interface}',member='{member}',path='{path}'"
        );
        self.connection
            .send
            .send_message_write_all(&add_match(&match_str))?;
        Ok(())
    }
}
