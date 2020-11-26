use std::net::{TcpListener, Shutdown};
use std::io;
use std::env;
use std::process;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::sync::mpsc::{self, Sender, Receiver};

use log::{error, warn, info, debug, trace, LevelFilter};
use ctrlc;

use chat_rs::{ChatStream, Msg, MSG_LENGTH};

const MAX_USERS: usize = 50;
type UsersType = Arc<Mutex<HashMap<String, ChatStream>>>;

fn main() -> io::Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    let address = env::args()
        .nth(1)
        .unwrap_or_else(|| {
            warn!("Bind IP missing, assuming 0.0.0.0");
            "0.0.0.0".into()
        });
    
    info!("Listening to connections on {}:7878", address);
    let listener = TcpListener::bind(format!("{}:7878", address))
        .unwrap_or_else(|err| {
            error!("Error on binding listener: {}", err.to_string());
            process::exit(1);
        });
    
    let users: UsersType = Arc::from(Mutex::from(HashMap::with_capacity(MAX_USERS)));

    let uclone: UsersType = users.clone();
    let rclone = running.clone();
    ctrlc::set_handler(move || {
        rclone.store(false, Ordering::SeqCst);
        info!("Received CTRL+C, exiting...");

        let users = uclone.lock().unwrap();
        debug!("Acquired users lock");
        for (nick, stream) in users.iter() {
            debug!("Shutting down {}'s stream", nick);
            match stream.0.shutdown(Shutdown::Both) {
                Ok(_) => trace!("Stream shutdown successful."),
                Err(_) => trace!("Stream shutdown failed.")
            }
        }
        process::exit(0);
    }).unwrap();

    let (tx, rx) = mpsc::channel();
    let uclone = users.clone();

    thread::spawn(move || {
       route_messages(rx, users); 
    });
    accept_connections(listener, uclone, running.clone(), tx);

    loop {} // ensures that main waits for ctrlc handler to finish
}

fn route_messages(rx: Receiver<(Msg, Option<String>)>, users: UsersType) {
    loop {
        let (msg, recepient) = rx.recv().unwrap();
        if let None = recepient { // message is to be broadcasted
            let mut users = users.lock().unwrap();
            for stream in users.values_mut() {
                stream.send_data(&msg).unwrap_or(()); // ignore failed sends
            }
        }
    }
}

fn accept_connections(listener: TcpListener, users: UsersType, running: Arc<AtomicBool>, tx: Sender<(Msg, Option<String>)>) {
    for stream in listener.incoming() {
        if !running.load(Ordering::SeqCst) {
            break
        }
        match stream {
            Ok(stream) => {
                let uclone = users.clone();
                let tx = tx.clone();
                thread::spawn(move || {
                    handle_connection(ChatStream(stream), uclone, tx);
                });
            },
            Err(_) => continue
        }
    }
}

fn handle_connection(mut stream: ChatStream, users: UsersType, tx: Sender<(Msg, Option<String>)>) {
    let peer_address = stream.peer_addr().unwrap();
    debug!("Incoming connection from {}", peer_address);

    let mut buffer = [0; MSG_LENGTH];

    let nick = match stream.receive_data(&mut buffer) {
        Ok(Msg::NickChange(nick)) => nick,
        _ => {
            warn!("{} aborted on nick.", peer_address);
            return
        }
    };

    { // lock users temporarily
        let userlock = users.lock().unwrap();
        if userlock.len() >= MAX_USERS {
            stream.send_data(&Msg::ConnectionRejected("too many users".into()))
                .unwrap_or_else(|_| {}); // do nothing, we don't need the user anyway
            info!("Rejected {}, too many users", peer_address);
            return
        } else if userlock.contains_key(&nick) {
            stream.send_data(&Msg::ConnectionRejected("nick taken".into()))
                .unwrap_or_else(|_| {}); // do nothing, we don't need the user anyway
            info!("Rejected {}, nick taken", peer_address);
            return
        }
    }

    if let Err(e) = stream.send_data(&Msg::ConnectionAccepted) {
        warn!("Error accepting {}: {}", peer_address, e.to_string());
        return
    }

    info!("Connection successful from {}, nick {}", peer_address, nick);
    tx.send((Msg::NickedConnect(nick.clone()), None)).unwrap();

    let stream_clone = match stream.try_clone() {
        Ok(stream_clone) => stream_clone,
        Err(e) => {
            error!("Couldn't clone stream for {}: {}", peer_address, e.to_string());
            return
        }
    };

    users.lock().unwrap().insert(nick.clone(), stream_clone);


    loop {
        let msg = match stream.receive_data(&mut buffer) {
            Ok(msg) => msg,
            Err(e) => {
                info!("{} [{}] disconnected.", peer_address, nick);
                debug!("Associated error: {}", e.to_string());
                users.lock().unwrap().remove(&nick);
                tx.send((Msg::NickedDisconnect(nick), None)).unwrap();
                break
            }
        };

        trace!("Msg({}): [{}]: {}", msg.code(), nick, msg.string());
        match msg {
            Msg::UserMsg(s) => tx.send((Msg::NickedUserMsg(nick.clone(), s), None)),
            Msg::NickChange(s) => tx.send((Msg::NickedNickChange(nick.clone(), s), None)),
            Msg::Command(s) => tx.send((Msg::NickedCommand(nick.clone(), s), None)),
            _ => Ok(())
        }.unwrap();
    }
}