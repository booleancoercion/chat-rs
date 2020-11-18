use std::net::{TcpStream, TcpListener};
use std::io::{self, prelude::*};
use std::env;
use std::process;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;

use chat_rs::{User, MSG_LENGTH};

fn main() -> io::Result<()>{
    let address = env::args()
        .nth(1)
        .unwrap_or_else(|| {
            eprintln!("Please pass an IP address to listen on.");
            process::exit(1);
        });
    
    println!("Listening to connections on {}:7878", address);
    let listener = TcpListener::bind(format!("{}:7878", address))
        .unwrap_or_else(|err| {
            eprintln!("Error on binding listener: {}", err.to_string());
            process::exit(1);
        });
    
    let users = Arc::from(Mutex::from(HashSet::with_capacity(50)));
    
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_connection(stream, users.clone());
            },
            Err(_) => continue
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, users: Arc<Mutex<HashSet<User>>>) {
    thread::spawn(move || {
        let peer_address = stream.peer_addr().unwrap();
        let mut buffer = [0u8; MSG_LENGTH];

        let nick = match receive_data(&mut buffer, &mut stream) {
            Ok((_, nick)) => nick,
            Err(_) => {
                println!("{} disconnected.", peer_address);
                return
            }
        };

        println!("Connection from {}, nick {}", peer_address, nick);

        let temp_user = User {
            nick: nick.clone(),
            stream: stream.try_clone()
                .expect(&format!("Couldn't clone stream for {}", peer_address))
        };
        users.lock().unwrap().insert(temp_user);


        loop {
            let (code, string) = match receive_data(&mut buffer, &mut stream) {
                Ok((code, string)) => (code, string),
                Err(_) => {
                    println!("{} [{}] disconnected.", peer_address, nick);
                    users.lock().unwrap().remove(&User {
                        nick,
                        stream
                    });
                    break
                }
            };

            println!("Msg({}): [{}]: {}", code, nick, string);
        }
    });
}

fn receive_data(buffer: &mut [u8], stream: &mut TcpStream) -> io::Result<(u8, String)> {
    let bytes_read = match stream.read(buffer) {
        Ok(num) if num > 0 => num,
        Ok(_) => return Err(io::Error::new(io::ErrorKind::Other, "Received empty message")),
        Err(err) => return Err(err)
    };

    let code = buffer[0];
    let string = String::from_utf8_lossy(&buffer[1..bytes_read]).to_string();

    Ok((code, string))
}