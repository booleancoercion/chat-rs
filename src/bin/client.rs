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
    
    let mut stream = ChatStream{stream};
    stream.send_data(Msg::NickChange(nick.clone()))?;
    println!("Connected.");
    
    loop {
        let mut string = prompt()?;
        string.truncate(MSG_LENGTH - 1);

        if string == "exit" {
            break;
        }
        
        stream.send_data(Msg::UserMsg(string))?;
    };
    io::Result::Ok(())
}

fn prompt() -> io::Result<String> {
    let mut string = String::with_capacity(MSG_LENGTH + 1);
    io::stdin().read_line(&mut string)?;
    Ok(string.trim().to_string())
}

fn prompt_msg(string: &str) -> io::Result<String> {
    print!("{}", string);
    io::stdout().flush()?;
    prompt()
}