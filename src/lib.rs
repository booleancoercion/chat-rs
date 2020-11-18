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

pub struct ChatStream{
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
    pub fn send_data(&mut self, code: u8, string: &str) -> io::Result<()> {
        let mut buffer = Vec::with_capacity(MSG_LENGTH);
        buffer.push(code);
        buffer.extend(string.as_bytes());
    
        self.stream.write_all(&buffer)?;
        self.stream.flush()?;
        io::Result::Ok(())
    }

    pub fn receive_data(&mut self, buffer: &mut [u8]) -> io::Result<(u8, String)> {
        let bytes_read = match self.stream.read(buffer) {
            Ok(num) if num > 0 => num,
            Ok(_) => return Err(io::Error::new(io::ErrorKind::Other, "Received empty message")),
            Err(err) => return Err(err)
        };
    
        let code = buffer[0];
        let string = String::from_utf8_lossy(&buffer[1..bytes_read]).to_string();
    
        Ok((code, string))
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self{
            stream: self.stream.try_clone()?
        })
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr>{
        self.stream.peer_addr()
    }
}