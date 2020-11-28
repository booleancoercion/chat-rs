//! Crate containing the main logic for the rust implementation for BCMP.
//!
//! This crate contains useful structs, methods and enums for dealing with BCMP
//! messages, e.g. `ChatStream` and `Msg`.

use std::io::{self, prelude::*};
use std::net::{SocketAddr, TcpStream};

/// The default maximum message length used between the
/// client and the server, according to BCMP.
pub const MSG_LENGTH: usize = 513; // 1+512 for block header

/// A struct representing a `TcpStream` belonging to a chat session.
/// This struct contains methods useful for sending and receiving information
/// using BCMP, and is highly recommended for working consistently between the
/// server and the client.
pub struct ChatStream {
    pub inner: TcpStream,
    key: Option<[u8; 32]> // 256-bit key
}

impl ChatStream {

    /// Generate a new ChatStream from an existing TcpStream, without encryption (Use ChatStream::encrypt
    /// to add a key).
    pub fn new(stream: TcpStream) -> Self {
        ChatStream {
            inner: stream,
            key: None
        }
    }

    /// Assigns a new encryption key used for communication. Note that this must be done
    /// on the receiving side as well, otherwise communication will be malformed.
    pub fn encrypt(&mut self, key: [u8; 32]) {
        self.key = Some(key);
    }

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
    ///     let mut stream = ChatStream::new(stream);
    ///     
    ///     stream.send_data(Msg::ConnectionAccepted)?;
    /// 
    ///     Ok(())
    /// }
    /// ```
    pub fn send_data(&mut self, msg: &Msg) -> io::Result<()> {
        let mut buffer = Vec::with_capacity(MSG_LENGTH);
        buffer.extend(&msg.encode_header());
        buffer.extend(msg.string().as_bytes());
        
        if buffer.len() > MSG_LENGTH {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Attempted to send an invalid-length message (too big)"));
        }
        self.inner.write_all(&buffer)?;
        self.inner.flush()?;
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
        self.inner.read_exact(&mut buffer[0..3])?;
        let (code, length) = Msg::parse_header(&buffer[0..3]);

        if length + 3 > MSG_LENGTH {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Received invalid message length (too big)"));
        }
        self.inner.read_exact(&mut buffer[3..length+3])?;
        let string = String::from_utf8_lossy(&buffer[3..length+3]).to_string();
    
        match Msg::from_parts(code, string) {
            Some(msg) => Ok(msg),
            None => Err(io::Error::new(io::ErrorKind::Other, "Received invalid message code"))
        }
    }

    /// Tries to clone itself using `TcpStream::try_clone()` on the underlying stream.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(ChatStream { inner: self.inner.try_clone()?, key: self.key })
    }

    /// Convenience method for `TcpStream::peer_addr()`
    pub fn peer_addr(&self) -> io::Result<SocketAddr>{
        self.inner.peer_addr()
    }
}

/// An enum representing a Server/Client message
#[derive(Clone)]
pub enum Msg {
    UserMsg(String),
    NickedUserMsg(String, String),

    NickChange(String),
    NickedNickChange(String, String),

    NickedConnect(String),
    NickedDisconnect(String),

    Command(String),
    NickedCommand(String, String),

    ConnectionEncrypted,
    ConnectionAccepted,
    ConnectionRejected(String),
}

impl Msg {
    /// Returns the numeral code of the message type.
    pub fn code(&self) -> u8 { // if you change this, CHANGE FROM_PARTS AND STRING TOO!!
        use Msg::*;
        match self {
            UserMsg(_) => 0,
            NickedUserMsg(_, _) => 100,

            NickChange(_) => 1,
            NickedNickChange(_, _) => 101,
            
            NickedConnect(_) => 98,
            NickedDisconnect(_) => 99,

            Command(_) => 3,
            NickedCommand(_, _) => 103,
            
            ConnectionEncrypted => 253,
            ConnectionAccepted => 254,
            ConnectionRejected(_) => 255,
        }
    }

    /// Constructs a new Msg from a code and a string.
    /// Msg's that don't have a string will ignore the passed string.
    pub fn from_parts(code: u8, string: String) -> Option<Self> {
        use Msg::*;
        match code {
            0 => Some(UserMsg(string)),
            1 => Some(NickChange(string)),
            98 => Some(NickedConnect(string)),
            99 => Some(NickedDisconnect(string)),
            3 => Some(Command(string)),
            253 => Some(ConnectionEncrypted),
            254 => Some(ConnectionAccepted),
            255 => Some(ConnectionRejected(string)),
            _ => {
                let (a, b) = match Self::nicked_split(string) {
                    Some((a, b)) => (a, b),
                    None => return None
                };
                match code {
                    100 => Some(NickedUserMsg(a, b)),
                    101 => Some(NickedNickChange(a, b)),
                    103 => Some(NickedCommand(a, b)),
                    _ => None
                }
            }
        }
    }

    fn nicked_split(string: String) -> Option<(String, String)> {
        let split_point = match string.find('\0') {
            Some(n) => n,
            None => return None
        };
        let (nick, other) = string.split_at(split_point);
        Some((nick.into(), other[1..].into()))
    }

    fn nicked_join(nick: &str, other: &str) -> String {
        let mut output = nick.to_string();
        output.push('\0');
        output.extend(other.chars());
        output
    }

    /// Returns the underlying string of the message.
    /// This method also contains defaults for string-less messages,
    /// e.g. `Msg::ConnectionAccepted`.
    pub fn string(&self) -> String {
        use Msg::*;
        match self {
            UserMsg(s) => s.to_string(),
            NickedUserMsg(n, s) => Self::nicked_join(n, s),

            NickChange(s) => s.to_string(),
            NickedNickChange(n, s) => Self::nicked_join(n, s),
            
            NickedConnect(n) => n.to_string(),
            NickedDisconnect(n) => n.to_string(),

            Command(s) => s.to_string(),
            NickedCommand(n, s) => Self::nicked_join(n, s),
            
            ConnectionEncrypted => String::from("connection encrypted; commence ECDH"),
            ConnectionAccepted => String::from("connection accepted"),
            ConnectionRejected(s) => s.to_string(),
        }
    }

    /// Parses a raw BCMP header into a message code and length.
    /// NOTE: This will read the header incorrectly when used with EBCMP!
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