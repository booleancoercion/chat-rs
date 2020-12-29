use std::collections::HashMap;
use std::env;
use std::io;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use log::{debug, error, info, trace, warn, LevelFilter};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::Mutex;

use chat_rs::*;

const MAX_USERS: usize = 50;
type UsersType = Arc<Mutex<HashMap<String, ChatWriterHalf>>>;

#[tokio::main]
async fn main() -> io::Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    let address = env::args().nth(1).unwrap_or_else(|| {
        warn!("Bind IP missing, assuming 0.0.0.0");
        "0.0.0.0".into()
    });

    let is_encrypted = env::var("CHAT_RS_UNENCRYPTED").is_err();
    if is_encrypted {
        info!("This server only accepts encrypted connections.")
    } else {
        info!("This server is operating in unencrypted mode.")
    }

    info!("Listening to connections on {}:7878", address);
    let listener = TcpListener::bind(format!("{}:7878", address))
        .await
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

        let uclone = uclone.clone();
        debug!("Acquired users lock");
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let mut users = uclone.lock().await;
                for (nick, writer) in users.iter_mut() {
                    debug!("Shutting down {}'s stream", nick);
                    let (mut inner, _) = writer.get_writer_cipher();

                    tokio::io::AsyncWriteExt::shutdown(&mut inner)
                        .await
                        .unwrap_or(());
                }
                process::exit(0);
            });
    })
    .unwrap();

    let (tx, rx) = mpsc::channel(32);
    let uclone = users.clone();

    tokio::spawn(async move {
        route_messages(rx, users).await;
    });
    accept_connections(listener, uclone, running.clone(), tx, is_encrypted).await;

    loop {
        std::thread::yield_now()
    } // ensures that main waits for ctrlc handler to finish
}

async fn route_messages(mut rx: Receiver<(Msg, Option<String>)>, users: UsersType) {
    loop {
        let (msg, recepient) = rx.recv().await.unwrap();
        if recepient.is_none() {
            // message is to be broadcasted
            let mut users = users.lock().await;
            for stream in users.values_mut() {
                stream.send_msg(&msg).await.unwrap_or(()); // ignore failed sends
            }
        }
    }
}

async fn accept_connections(
    listener: TcpListener,
    users: UsersType,
    running: Arc<AtomicBool>,
    tx: Sender<(Msg, Option<String>)>,
    is_encrypted: bool,
) {
    loop {
        if !running.load(Ordering::SeqCst) {
            break;
        }
        if let Ok((stream, _)) = listener.accept().await {
            let uclone = users.clone();
            let tx = tx.clone();
            tokio::spawn(async move {
                handle_connection(ChatStream::new(stream), uclone, tx, is_encrypted).await;
            });
        }
    }
}

async fn handle_connection(
    mut stream: ChatStream,
    users: UsersType,
    tx: Sender<(Msg, Option<String>)>,
    is_encrypted: bool,
) {
    let peer_address = stream.peer_addr().unwrap();
    debug!("Incoming connection from {}", peer_address);

    let mut buffer = [0; MSG_LENGTH];

    let nick = match stream.receive_msg(&mut buffer).await {
        Ok(Msg::NickChange(nick)) => nick,
        _ => {
            warn!("{} aborted on nick.", peer_address);
            return;
        }
    };

    {
        // lock users temporarily
        let userlock = users.lock().await;
        if userlock.len() >= MAX_USERS {
            stream
                .send_msg(&Msg::ConnectionRejected("too many users".into()))
                .await
                .unwrap_or(()); // do nothing, we don't need the user anyway
            info!("Rejected {}, too many users", peer_address);
            return;
        } else if userlock.contains_key(&nick) {
            stream
                .send_msg(&Msg::ConnectionRejected("nick taken".into()))
                .await
                .unwrap_or(()); // do nothing, we don't need the user anyway
            info!("Rejected {}, nick taken", peer_address);
            return;
        }
    }
    let msg = if is_encrypted {
        Msg::ConnectionEncrypted
    } else {
        Msg::ConnectionAccepted
    };

    if let Err(e) = stream.send_msg(&msg).await {
        warn!("Error accepting {}: {}", peer_address, e.to_string());
        return;
    }

    if is_encrypted {
        stream.encrypt().await.unwrap();
        debug!("Encrypted stream from {}", peer_address);
    }

    info!("Connection successful from {}, nick {}", peer_address, nick);
    tx.send((Msg::NickedConnect(nick.clone()), None))
        .await
        .unwrap();

    let (mut reader, writer) = stream.into_split();
    users.lock().await.insert(nick.clone(), writer);

    loop {
        let msg = match reader.receive_msg(&mut buffer).await {
            Ok(msg) => msg,
            Err(e) => {
                info!("{} [{}] disconnected.", peer_address, nick);
                debug!("Associated error: {}", e.to_string());
                users.lock().await.remove(&nick);
                tx.send((Msg::NickedDisconnect(nick), None)).await.unwrap();
                break;
            }
        };

        trace!("Msg({}): [{}]: {}", msg.code(), nick, msg.string());
        match msg {
            Msg::UserMsg(s) => tx.send((Msg::NickedUserMsg(nick.clone(), s), None)).await,
            Msg::NickChange(s) => {
                tx.send((Msg::NickedNickChange(nick.clone(), s), None))
                    .await
            }
            Msg::Command(s) => tx.send((Msg::NickedCommand(nick.clone(), s), None)).await,
            _ => Ok(()),
        }
        .unwrap();
    }
}
