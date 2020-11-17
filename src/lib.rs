use std::{hash::Hash, net::TcpStream};

pub const MSG_LENGTH: usize = 1024;

pub struct User {
    pub nick: String,
    pub stream: TcpStream
}

impl PartialEq for User {
    fn eq(&self, other: &Self) -> bool {
        self.stream.peer_addr().unwrap() == other.stream.peer_addr().unwrap()
    }
}

impl Eq for User {}

impl Hash for User {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.stream.peer_addr().unwrap().hash(state)
    }
}