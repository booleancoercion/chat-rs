use std::net::TcpStream;
use std::io::{self, prelude::*};
use std::env;
use std::process;

use chat_rs::{ChatStream, Msg, MSG_LENGTH};

fn main() -> io::Result<()> {
    let address = env::args()
        .nth(1)
        .unwrap_or_else(|| {
            eprintln!("Please pass an IP address to connect to.");
            process::exit(1);
        });
    
    println!("Connecting to {}:7878", address);

    
    let nick = prompt_msg("Enter nickname: ")?;

    let stream = TcpStream::connect(format!("{}:7878", address))
        .unwrap_or_else(|err| {
            eprintln!("Error on connecting: {}", err.to_string());
            process::exit(1);
        });
    
    let mut stream = ChatStream(stream);
    let mut buffer = [0u8; MSG_LENGTH];
    
    match stream.receive_data(&mut buffer) {
        Ok(Msg::ConnectionAccepted) => println!("Connected. Sending nickname..."),
        Ok(msg) => println!("Server refused connection: {}", msg.string()),
        Err(e) => println!("Error connecting to server: {}", e.to_string())
    }

    stream.send_data(Msg::NickChange(nick.clone()))?;
    println!("Connected.");
    
    
    loop {
        let string = prompt()?;

        if string == "exit" {
            break;
        }
        
        stream.send_data(Msg::UserMsg(string))?;
    };
    io::Result::Ok(())
}


/// Prompts the user for a string via stdin, **without** a message.
fn prompt() -> io::Result<String> {
    let mut string = String::with_capacity(MSG_LENGTH + 1);
    io::stdin().read_line(&mut string)?;
    Ok(string.trim().to_string())
}

/// Prompts the user for a string via stdin, **with** a message.
fn prompt_msg(string: &str) -> io::Result<String> {
    print!("{}", string);
    io::stdout().flush()?;
    prompt()
}