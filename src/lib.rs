//! Crate containing the main logic for the rust implementation for BCMP.
//!
//! This crate contains useful structs, methods and enums for dealing with BCMP
//! messages, e.g. `ChatStream` and `Msg`.

use std::io::{self, prelude::*};
use std::net::{SocketAddr, TcpStream};

/// The default maximum message length used between the
/// client and the server, according to BCMP.
pub const MSG_LENGTH: usize = 2048;

/// A struct representing a `TcpStream` belonging to a chat session.
/// This struct contains methods useful for sending and receiving information
/// using BCMP, and is highly recommended for working consistently between the
/// server and the client.
pub struct ChatStream(pub TcpStream);

impl ChatStream {
    /// Send a message using the contained `TcpStream`, formatted according to
    /// BCMP, and returns a result which states if the operation was
    /// successful.
    /// 
    /// # Examples
    /// 
    /// Accepting a connection from a client:
    /// ```
    /// use std::net::TcpListener;
    /// use chat_rs::{Msg, ChatStream};
    /// 
    /// fn main() -> std::io::Result<()> {
    ///     let listener = TcpListener::bind("0.0.0.0:7878")?;
    /// 
    ///     let (stream, _) = listener.accept()?;
    ///     let mut stream = ChatStream(stream);
    ///     
    ///     stream.send_data(Msg::ConnectionAccepted)?;
    /// 
    ///     Ok(())
    /// }
    /// ```
    pub fn send_data(&mut self, msg: Msg) -> io::Result<()> {
        let mut buffer = Vec::with_capacity(MSG_LENGTH);
        buffer.extend(&msg.encode_header());
        buffer.extend(msg.string().as_bytes());
        
        if buffer.len() > MSG_LENGTH {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Attempted to send an invalid-length message (too big)"));
        }
        self.0.write_all(&buffer)?;
        self.0.flush()?;
        io::Result::Ok(())
    }

    /// Receive a BCMP formatted message, using the provided buffer
    /// as a means for memory efficiency. Buffer must be of length `MSG_LENGTH` at least.
    /// 
    /// # Examples
    /// 
    /// Connecting to the server:
    /// ```
    /// use std::net::TcpStream;
    /// use chat_rs::{Msg, ChatStream, MSG_LENGTH};
    /// 
    /// fn main() -> std::io::Result<()> {
    ///     let stream = TcpStream::connect("127.0.0.1:7878")?;
    ///     let mut stream = ChatStream(stream);
    ///     
    ///     let mut buffer = [0u8; MSG_LENGTH];
    ///     let msg = stream.receive_data(&mut buffer)?;
    ///     // msg should be an accept/reject response
    /// 
    ///     Ok(())
    /// }
    /// ```
    pub fn receive_data(&mut self, buffer: &mut [u8]) -> io::Result<Msg> {
        self.0.read_exact(&mut buffer[0..3])?;
        let (code, length) = Msg::parse_header(&buffer[0..3]);

        if length + 3 > MSG_LENGTH {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Received invalid message length (too big)"));
        }
        self.0.read_exact(&mut buffer[3..length+3])?;
        let string = String::from_utf8_lossy(&buffer[3..length+3]).to_string();
    
        match Msg::from_parts(code, string) {
            Some(msg) => Ok(msg),
            None => Err(io::Error::new(io::ErrorKind::Other, "Received invalid message code"))
        }
    }

    /// Tries to clone itself using `TcpStream::try_clone()` on the underlying stream.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(ChatStream(self.0.try_clone()?))
    }

    /// Convenience method for `TcpStream::peer_addr()`
    pub fn peer_addr(&self) -> io::Result<SocketAddr>{
        self.0.peer_addr()
    }
}

/// An enum representing a Server/Client message
#[derive(Clone)]
pub enum Msg {
    UserMsg(String),
    NickChange(String),
    TooManyUsers,
    ConnectionAccepted,
}

impl Msg {
    /// Returns the numeral code of the message type.
    pub fn code(&self) -> u8 { // if you change this, CHANGE FROM_PARTS AND STRING TOO!!
        use Msg::*;
        match self {
            UserMsg(_) => 0,
            NickChange(_) => 1,
            ConnectionAccepted => 254,
            TooManyUsers => 255,
        }
    }

    /// Constructs a new Msg from a code and a string.
    /// Msg's that don't have a string will ignore the passed string.
    pub fn from_parts(code: u8, string: String) -> Option<Self> {
        use Msg::*;
        match code {
            0 => Some(UserMsg(string)),
            1 => Some(NickChange(string)),
            254 => Some(ConnectionAccepted),
            255 => Some(TooManyUsers),
            _ => None
        }
    }

    /// Returns the underlying string of the message.
    /// This method also contains defaults for string-less messages,
    /// e.g. `Msg::ConnectionAccepted`.
    pub fn string(&self) -> String {
        match self {
            Self::UserMsg(s) => s,
            Self::NickChange(s) => s,
            Self::TooManyUsers => "too many users",
            Self::ConnectionAccepted => "connection accepted"
        }.to_string()
    }

    /// Parses a raw BCMP header into a message code and length. 
    pub fn parse_header(header: &[u8]) -> (u8, usize) {
        let code = header[0];
        let length = u16::from_le_bytes([header[1], header[2]]) as usize;

        (code, length)
    }

    /// Encodes the header of the current message.
    pub fn encode_header(&self) -> [u8; 3] {
        let mut out = [0u8; 3];
        out[0] = self.code();
        let le = (self.string().len() as u16).to_le_bytes();
        out[1] = le[0];
        out[2] = le[1];
        out
    }
}