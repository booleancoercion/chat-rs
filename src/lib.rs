use std::hash::Hash;
use std::io::{self, prelude::*};
use std::net::{SocketAddr, TcpStream};

pub const MSG_LENGTH: usize = 2048;

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
    /// Send a message using the contained TcpStream, formatted according to
    /// BCMP, and returns a result which states if the operation was
    /// successful.
    pub fn send_data(&mut self, msg: Msg) -> io::Result<()> {
        let mut buffer = Vec::with_capacity(MSG_LENGTH);
        buffer.extend(&msg.encode_header());
        buffer.extend(msg.string().as_bytes());
        
        if buffer.len() > MSG_LENGTH {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Attempted to send an invalid-length message (too big)"));
        }
        self.stream.write_all(&buffer)?;
        self.stream.flush()?;
        io::Result::Ok(())
    }

    /// Receive a BCMP formatted message, appending to the provided buffer
    /// as a means for memory efficiency.
    /// 
    /// Note that the buffer will be emptied.
    pub fn receive_data(&mut self, buffer: &mut Vec<u8>) -> io::Result<Msg> {
        buffer.clear();

        self.stream.read_exact(&mut buffer[0..3])?;
        let (code, length) = Msg::parse_header(&buffer[0..3]);
        if length + 3 > MSG_LENGTH {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Received invalid message length (too big)"));
        }

        self.stream.read_exact(&mut buffer[3..length+3])?;
        let string = String::from_utf8_lossy(&buffer[3..length+3]).to_string();
    
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
    pub fn code(&self) -> u8 { // if you change this, CHANGE FROM_PARTS AND STRING TOO!!
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

    pub fn parse_header(header: &[u8]) -> (u8, usize) {
        let code = header[0];
        let length = u16::from_le_bytes([header[1], header[2]]) as usize;

        (code, length)
    }

    pub fn encode_header(&self) -> [u8; 3] {
        let mut out = [0u8; 3];
        out[0] = self.code();
        let le = (self.string().len() as u16).to_le_bytes();
        out[1] = le[0];
        out[2] = le[1];
        out
    }
}