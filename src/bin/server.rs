use std::net::{TcpStream, TcpListener};
use std::io;
use std::env;
use std::process;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;

use chat_rs::{ChatStream, User, Msg, MSG_LENGTH};

const MAX_USERS: usize = 50;

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
    
    let users = Arc::from(Mutex::from(HashSet::with_capacity(MAX_USERS)));
    
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let uclone = users.clone();
                thread::spawn(move || {
                    handle_connection(stream, uclone);
                });
            },
            Err(_) => continue
        }
    }
    Ok(())
}

fn handle_connection(stream: TcpStream, users: Arc<Mutex<HashSet<User>>>) {
    let mut stream = ChatStream{stream};
    let peer_address = stream.peer_addr().unwrap();

    let mut buffer = [0u8; MSG_LENGTH];

    let nick = match stream.receive_data(&mut buffer) {
        Ok(Msg::NickChange(nick)) => nick,
        _ => {
            println!("{} aborted on nick.", peer_address);
            return
        }
    };

    if users.lock().unwrap().len() >= MAX_USERS {
        stream.send_data(Msg::TooManyUsers)
            .unwrap_or_else(|_| {}); // do nothing, we don't need the user anyway
        println!("Rejected {}, too many users", peer_address);
        return
    } else {
        stream.send_data(Msg::ConnectionAccepted).unwrap();
        println!("Connection from {}, nick {}", peer_address, nick);
    }

    let temp_user = User {
        nick: nick.clone(),
        stream: stream.try_clone()
            .expect(&format!("Couldn't clone stream for {}", peer_address))
    };
    users.lock().unwrap().insert(temp_user);


    loop {
        let msg = match stream.receive_data(&mut buffer) {
            Ok(msg) => msg,
            Err(e) => {
                println!("{} [{}] disconnected with error {}.", peer_address, nick, e.to_string());
                users.lock().unwrap().remove(&User {
                    nick,
                    stream
                });
                break
            }
        };

        println!("Msg({}): [{}]: {}", msg.code(), nick, msg.string());
    }
}