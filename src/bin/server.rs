use std::net::TcpListener;
use std::io;
use std::env;
use std::process;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use chat_rs::{ChatStream, Msg, MSG_LENGTH};

const MAX_USERS: usize = 50;

fn main() -> io::Result<()> {
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
    
    let users = Arc::from(Mutex::from(HashMap::with_capacity(MAX_USERS)));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let uclone = users.clone();

                thread::spawn(move || {
                    handle_connection(ChatStream(stream), uclone);
                });
            },
            Err(_) => continue
        }
    }
    Ok(())
}

fn handle_connection(mut stream: ChatStream, users: Arc<Mutex<HashMap<String, ChatStream>>>) {
    let peer_address = stream.peer_addr().unwrap();

    if users.lock().unwrap().len() >= MAX_USERS {
        stream.send_data(Msg::TooManyUsers)
            .unwrap_or_else(|_| {}); // do nothing, we don't need the user anyway
        println!("Rejected {}, too many users", peer_address);
        return
    }

    let mut buffer = [0; MSG_LENGTH];

    let nick = match stream.receive_data(&mut buffer) {
        Ok(Msg::NickChange(nick)) => nick,
        _ => {
            println!("{} aborted on nick.", peer_address);
            return
        }
    };

    stream.send_data(Msg::ConnectionAccepted).unwrap();
    println!("Connection from {}, nick {}", peer_address, nick);

    let stream_clone = stream.try_clone()
        .expect(&format!("Couldn't clone stream for {}", peer_address));

    users.lock().unwrap().insert(nick.clone(), stream_clone);


    loop {
        let msg = match stream.receive_data(&mut buffer) {
            Ok(msg) => msg,
            Err(e) => {
                println!("{} [{}] disconnected with error {}.", peer_address, nick, e.to_string());
                users.lock().unwrap().remove(&nick);
                break
            }
        };

        println!("Msg({}): [{}]: {}", msg.code(), nick, msg.string());
    }
}