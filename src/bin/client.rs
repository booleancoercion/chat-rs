use std::net::TcpStream;
use std::io::{self, prelude::*};
use std::env;
use std::process;
use std::error::Error;
use std::thread;
use std::sync::atomic::{AtomicU16, Ordering};

#[allow(unused_imports)]
use crossterm::{execute, queue};

use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{self, ClearType};
use crossterm::style::{self, Colorize};

use chat_rs::{ChatStream, Msg, MSG_LENGTH};

static input_rows: AtomicU16 = AtomicU16::new(1);
static messages: Vec<String> = Vec::new();

fn main() -> Result<(), Box<dyn Error>> {
    let address = env::args()
        .nth(1)
        .unwrap_or_else(|| {
            prompt_msg("Please input the server IP: ").unwrap()
        });
    
    println!("Connecting to {}:7878", address);

    let mut stream = connect_stream(address).unwrap_or_else(|err| {
        eprintln!("Error on connecting: {}", err.to_string());
        process::exit(1);
    });
    let nick = prompt_msg("Enter nickname: ")?;

    let mut buffer = [0u8; MSG_LENGTH];
    
    stream.send_data(Msg::NickChange(nick.clone()))?;

    match stream.receive_data(&mut buffer) {
        Ok(Msg::ConnectionAccepted) => println!("Connected."),
        Ok(msg) => {
            eprintln!("Server refused connection: {}", msg.string());
            process::exit(0)
        },
        Err(e) => {
            println!("Error connecting to server: {}", e.to_string());
            process::exit(0)
        }
    }
    
    thread::spawn({
        let stream_clone = stream.try_clone()?;
        || { listen(stream_clone) }
    });

    handle_input(stream)?;
    Ok(())
}

fn connect_stream(address: String) -> Result<ChatStream, io::Error> {
    let stream = TcpStream::connect(format!("{}:7878", address))?;
    Ok(ChatStream(stream))
}

fn listen(mut stream: ChatStream) {
    let mut buffer = [0u8; MSG_LENGTH];
    let mut stdout = io::stdout();
    loop {
        let msg = match stream.receive_data(&mut buffer) {
            Err(_) => {
                execute!(stdout, terminal::LeaveAlternateScreen).unwrap();
                println!("Disconnected from server.");
                process::exit(0);
            },
            Ok(msg) => msg
        };
        
        match msg {
            Msg::UserMsg(string) => execute!(
                stdout,
            ).unwrap(),
            _ => {}
        }
    }
}

fn handle_input(mut stream: ChatStream) -> Result<(), Box<dyn Error>>{
    let mut stdout = io::stdout();

    let (mut x, mut y) = terminal::size()?;

    terminal::enable_raw_mode()?;
    execute!(stdout,
        terminal::EnterAlternateScreen,
        cursor::MoveTo(0, y))?;

    let mut string = String::new();
    loop {
        let event = event::read()?;
        if let Event::Key(event) = event {
            if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('c') {
                break
            } else if event.code == KeyCode::Enter && string.len() > 0 {
                stream.send_data(Msg::UserMsg(string.clone()))?;
                string.clear();
                execute!(stdout, terminal::Clear(ClearType::FromCursorUp), cursor::MoveTo(0,y))?;
                input_rows.store(1, Ordering::SeqCst);
                draw_messages()?;
            } else if event.code == KeyCode::Backspace && string.len() > 0 {
                string.pop();
                let (posx, posy) = cursor::position()?;
                if posx == 0 {
                    execute!(stdout, cursor::MoveTo(x, posy-1), style::Print(' '), terminal::ScrollDown(1), cursor::MoveTo(x, posy))?;
                    input_rows.fetch_sub(1, Ordering::SeqCst);
                } else {
                    execute!(stdout, cursor::MoveLeft(1), style::Print(' '), cursor::MoveLeft(1))?;
                }
                draw_messages()?;
            } else if let KeyCode::Char(c) = event.code {
                if !event.modifiers.contains(KeyModifiers::CONTROL) {
                    string.push(c);
                    execute!(stdout, style::Print(c))?;
                    let (posx, _) = cursor::position()?;
                    if posx == 0 {
                        input_rows.fetch_add(1, Ordering::SeqCst);
                    }
                    draw_messages()?;
                }
            } 
        } else if let Event::Resize(x0, y0) = event {
            x = x0;
            y = y0;
        }
    }

    execute!(stdout, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    Ok(())
}

fn draw_messages() -> Result<(), Box<dyn Error>> {

    Ok(())
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