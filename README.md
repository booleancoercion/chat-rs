# chat-rs
A client-server chat platform implemented in rust.

This crate contains the library implementing the `chat-rs` protocol. For the server and client implementations,
see the `server`, `client_term` and `client_gui` directories respectively.

## The Protocol
Communication between the client and the server is done using *Messages*.  
Not to be confused with a chat message, a BCMP message includes a header with information about the message contents, and
then optionally additional message contents as needed.

### Headers
A message's header consists of 3 bytes - first comes the *discriminant*, and then two (big endian) bytes describing
the length of the following contents.
Messages with no contents have a length of 0.

The discriminant distinguishes between different types of messages. For a comprehensive list of message types,
see the `Msg` enum in this crate's source.

### Message Contents
A message can optionally contain a UTF-8 encoded string. Nicked messages (as in, messages that come from the server and contain nickname information) first store the nickname, then a null byte, and then the rest of the message.

## Encrypted Protocol Extension
For security and coherency reasons, encrypted messages are encoded in a slightly different way.

First, the message is encoded as normal into bytes. Then, the number of *blocks* is determined, where a block is a sequence of exactly 16 bytes.
If the *total* message length (i.e. including the header) is not evenly divisible by 16, padding bytes are included (this implementation uses zero-padding,
any padding is fine).

The padded message is then encrypted using AES-256, and the number of blocks is appended to the beginning of the encrypted message as a single byte.
