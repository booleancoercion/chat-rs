//! Crate containing the main logic for the rust implementation for BCMP.
//!
//! This crate contains useful structs, methods and enums for dealing with BCMP
//! messages, e.g. `ChatStream` and `Msg`.

use std::net::SocketAddr;

use aes::cipher::{generic_array::GenericArray, BlockCipher, NewBlockCipher};
use aes::Aes256;
use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use hkdf::Hkdf;
use k256::{ecdh::EphemeralSecret, EncodedPoint};
use rand_core::OsRng;
use sha2::Sha256;
use tokio::net::{
    tcp::{OwnedReadHalf, OwnedWriteHalf},
    TcpStream,
};
use tokio::prelude::*;

/// The default maximum message length used between the
/// client and the server, according to BCMP.
pub const MSG_LENGTH: usize = 513; // 1+512 for block header
pub const ECDH_PUBLIC_LEN: usize = 33;

/// A struct representing a `TcpStream` belonging to a chat session.
/// This struct contains methods useful for sending and receiving information
/// using BCMP, and is highly recommended for working consistently between the
/// server and the client.
#[derive(Debug)]
pub struct ChatStream {
    pub inner: TcpStream,
    cipher: Option<Aes256>, // 256-bit key
}

#[async_trait]
pub trait SendMsg {
    type Writer: AsyncWrite + Unpin + Send;

    fn get_writer_cipher(&mut self) -> (&mut Self::Writer, Option<&Aes256>);

    /// Send a message using the contained `TcpStream`, formatted according to
    /// BCMP, and returns a result which states if the operation was
    /// successful.
    ///
    /// # Examples
    ///
    /// Accepting a connection from a client:
    /// ```no_run
    /// use tokio::net::TcpListener;
    /// use std::error::Error;
    /// use chat_rs::{Msg, ChatStream, SendMsg};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let listener = TcpListener::bind("0.0.0.0:7878").await?;
    ///
    ///     let (stream, _) = listener.accept().await?;
    ///     let mut stream = ChatStream::new(stream);
    ///     
    ///     stream.send_msg(&Msg::ConnectionAccepted).await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    async fn send_msg(&mut self, msg: &Msg) -> Result<()> {
        let (writer, cipher) = self.get_writer_cipher();

        let mut buffer = Vec::with_capacity(MSG_LENGTH);
        if cipher.is_some() {
            buffer.push(0);
        }
        buffer.extend(&msg.encode_header());
        buffer.extend(msg.string().as_bytes());

        if buffer.len() > MSG_LENGTH {
            bail!("Attempted to send an invalid-length message (too big)");
        }

        if let Some(cipher) = cipher {
            let msg_len = buffer.len() - 1;
            let blocks = {
                let temp = msg_len / 16;
                if temp * 16 < msg_len {
                    buffer.extend([0].repeat((temp + 1) * 16 - msg_len));
                    temp + 1
                } else {
                    temp
                }
            } as u8;

            buffer[0] = blocks;

            for chunk in (&mut buffer[1..]).chunks_mut(16) {
                let block = GenericArray::from_mut_slice(chunk);
                cipher.encrypt_block(block);
            }
        }
        writer.write_all(&buffer).await?;
        writer.flush().await?;
        Ok(())
    }
}

#[async_trait]
pub trait ReceiveMsg {
    type Reader: AsyncRead + Unpin + Send;

    fn get_reader_cipher(&mut self) -> (&mut Self::Reader, Option<&Aes256>);

    /// Receive a BCMP formatted message, using the provided buffer
    /// as a means for memory efficiency. Buffer must be of length `MSG_LENGTH` at least.
    ///
    /// # Examples
    ///
    /// Connecting to the server:
    /// ```no_run
    /// use tokio::net::TcpStream;
    /// use std::error::Error;
    /// use chat_rs::{Msg, ChatStream, MSG_LENGTH, ReceiveMsg};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let stream = TcpStream::connect("127.0.0.1:7878").await?;
    ///     let mut stream = ChatStream::new(stream);
    ///     
    ///     let mut buffer = [0u8; MSG_LENGTH];
    ///     let msg = stream.receive_msg(&mut buffer).await?;
    ///     // msg should be an accept/reject response
    ///
    ///     Ok(())
    /// }
    /// ```
    async fn receive_msg(&mut self, buffer: &mut [u8]) -> Result<Msg> {
        let (reader, cipher) = self.get_reader_cipher();

        let (code, length) = if let Some(cipher) = cipher {
            reader.read_exact(&mut buffer[0..1]).await?;
            let blocks = buffer[0] as usize;

            if blocks * 16 + 1 > MSG_LENGTH {
                bail!("Received invalid block amount (too big)");
            }

            reader.read_exact(&mut buffer[1..(1 + blocks * 16)]).await?;

            for chunk in (&mut buffer[1..]).chunks_mut(16) {
                let block = GenericArray::from_mut_slice(chunk);
                cipher.decrypt_block(block);
            }

            Msg::parse_header(&buffer[1..4])
        } else {
            reader.read_exact(&mut buffer[0..3]).await?;
            Msg::parse_header(&buffer[0..3])
        };

        if length + 3 > MSG_LENGTH {
            bail!("Received invalid message length (too big)");
        }

        let string = if cipher.is_some() {
            String::from_utf8_lossy(&buffer[4..length + 4]).to_string()
        } else {
            reader.read_exact(&mut buffer[3..length + 3]).await?;
            String::from_utf8_lossy(&buffer[3..length + 3]).to_string()
        };

        match Msg::from_parts(code, string) {
            Some(msg) => Ok(msg),
            None => Err(anyhow!("Received invalid message code")),
        }
    }
}

impl ChatStream {
    /// Generate a new ChatStream from an existing TcpStream, without encryption (Use ChatStream::encrypt
    /// to add a key).
    pub fn new(stream: TcpStream) -> Self {
        ChatStream {
            inner: stream,
            cipher: None,
        }
    }

    /// Encrypts the current ChatStream.
    /// NOTE: This operation must be executed on both ends to work.
    ///
    /// Calling this function when the stream is already encrypted
    /// will do nothing.
    pub async fn encrypt(&mut self) -> Result<()> {
        if self.cipher.is_some() {
            return Ok(());
        }
        let my_secret = EphemeralSecret::random(&mut OsRng);
        let my_public = EncodedPoint::from(&my_secret);

        let public_bytes = my_public.as_bytes(); // The length of this should be exactly ECDH_PUBLIC_LEN bytes
        self.inner.write_all(public_bytes).await?;
        self.inner.flush().await?;

        let mut other_public_bytes = [0u8; ECDH_PUBLIC_LEN];
        self.inner.read_exact(&mut other_public_bytes).await?;
        let other_public = EncodedPoint::from_bytes(&other_public_bytes)?;

        let shared = my_secret.diffie_hellman(&other_public)?;
        let shared_bytes = &shared.as_bytes()[..];
        let hk = Hkdf::<Sha256>::new(None, shared_bytes);

        let mut key = [0u8; 32];
        hk.expand(&[], &mut key)
            .expect("hk.expand got invalid length - this should never ever happen!");

        let key = GenericArray::from_slice(&key);
        self.cipher = Some(Aes256::new(&key));
        Ok(())
    }

    /// Convenience method for `TcpStream::peer_addr()`
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    /// Splits the current stream into a reading and writing half,
    /// using TcpStream::into_split
    pub fn into_split(self) -> (ChatReaderHalf, ChatWriterHalf) {
        let (read, write) = self.inner.into_split();

        let reader = ChatReaderHalf {
            inner: read,
            cipher: self.cipher.clone(),
        };

        let writer = ChatWriterHalf {
            inner: write,
            cipher: self.cipher,
        };

        (reader, writer)
    }
}

impl SendMsg for ChatStream {
    type Writer = TcpStream;

    fn get_writer_cipher(&mut self) -> (&mut Self::Writer, Option<&Aes256>) {
        (&mut self.inner, self.cipher.as_ref())
    }
}

impl ReceiveMsg for ChatStream {
    type Reader = TcpStream;

    fn get_reader_cipher(&mut self) -> (&mut Self::Reader, Option<&Aes256>) {
        (&mut self.inner, self.cipher.as_ref())
    }
}

pub struct ChatReaderHalf {
    inner: OwnedReadHalf,
    cipher: Option<Aes256>,
}

impl ReceiveMsg for ChatReaderHalf {
    type Reader = OwnedReadHalf;

    fn get_reader_cipher(&mut self) -> (&mut Self::Reader, Option<&Aes256>) {
        (&mut self.inner, self.cipher.as_ref())
    }
}

pub struct ChatWriterHalf {
    inner: OwnedWriteHalf,
    cipher: Option<Aes256>,
}

impl SendMsg for ChatWriterHalf {
    type Writer = OwnedWriteHalf;

    fn get_writer_cipher(&mut self) -> (&mut Self::Writer, Option<&Aes256>) {
        (&mut self.inner, self.cipher.as_ref())
    }
}

/// An enum representing a Server/Client message
#[derive(Debug, Clone)]
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
    pub fn code(&self) -> u8 {
        // if you change this, CHANGE FROM_PARTS AND STRING TOO!!
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
                    None => return None,
                };
                match code {
                    100 => Some(NickedUserMsg(a, b)),
                    101 => Some(NickedNickChange(a, b)),
                    103 => Some(NickedCommand(a, b)),
                    _ => None,
                }
            }
        }
    }

    fn nicked_split(string: String) -> Option<(String, String)> {
        let split_point = match string.find('\0') {
            Some(n) => n,
            None => return None,
        };
        let (nick, other) = string.split_at(split_point);
        Some((nick.into(), other[1..].into()))
    }

    fn nicked_join(nick: &str, other: &str) -> String {
        let mut output = nick.to_string();
        output.push('\0');
        output.push_str(other);
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
