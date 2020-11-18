use std::hash::Hash;
use std::io::{self, prelude::*};
use std::net::{SocketAddr, TcpStream};

pub const MSG_LENGTH: usize = 1024;

#[derive(Eq, Hash)]
pub struct User {
    pub nick: String,
    pub stream: ChatStream
}

impl PartialEq for User {
    fn eq(&self, other: &Self) -> bool {
        self.stream == other.stream
    }
}

impl User {
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(User {
            nick: self.nick.clone(),
            stream: self.stream.try_clone()?
        })
    }
}

pub struct ChatStream {
    pub stream: TcpStream
}

impl PartialEq for ChatStream {
    fn eq(&self, other: &Self) -> bool {
        self.stream.peer_addr().unwrap() == other.stream.peer_addr().unwrap()
    }
}

impl Eq for ChatStream {}

impl Hash for ChatStream {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.stream.peer_addr().unwrap().hash(state)
    }
}

impl ChatStream {
    /// Send a message using the contained TcpStream, formatted with a code
    /// and string, and returns a result which states if the operation was
    /// successful.
    pub fn send_data(&mut self, msg: Msg) -> io::Result<()> {
        let mut buffer = Vec::with_capacity(MSG_LENGTH);
        buffer.push(msg.code());
        buffer.extend(msg.string().as_bytes());
    
        self.stream.write_all(&buffer)?;
        self.stream.flush()?;
        io::Result::Ok(())
    }

    pub fn receive_data(&mut self, buffer: &mut [u8]) -> io::Result<Msg> {
        let bytes_read = match self.stream.read(buffer) {
            Ok(num) if num > 0 => num,
            Ok(_) => return Err(io::Error::new(io::ErrorKind::Other, "Received empty message")),
            Err(err) => return Err(err)
        };
    
        let code = buffer[0];
        let string = String::from_utf8_lossy(&buffer[1..bytes_read]).to_string();
    
        match Msg::from_parts(code, string) {
            Some(msg) => Ok(msg),
            None => Err(io::Error::new(io::ErrorKind::Other, "Received invalid message code"))
        }
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            stream: self.stream.try_clone()?
        })
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr>{
        self.stream.peer_addr()
    }
}

#[derive(Clone)]
pub enum Msg {
    UserMsg(String),
    NickChange(String),
    TooManyUsers,
    ConnectionAccepted,
}

impl Msg {
    pub fn code(&self) -> u8 { // if you change this, CHANGE FROM_PARTS TOO!!
        use Msg::*;
        match self {
            UserMsg(_) => 0,
            NickChange(_) => 1,
            ConnectionAccepted => 254,
            TooManyUsers => 255,
        }
    }

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

    pub fn string(&self) -> String {
        match self {
            Self::UserMsg(s) => s,
            Self::NickChange(s) => s,
            Self::TooManyUsers => "too many users",
            Self::ConnectionAccepted => "connection accepted"
        }.to_string()
    }
}