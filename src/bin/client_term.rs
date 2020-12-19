use std::env;
use std::error::Error;
use std::io::{self, prelude::*};
use std::process;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::style::{self, Attribute, Colorize};
use crossterm::terminal::{self, ClearType};
#[allow(unused_imports)]
use crossterm::{execute, queue};
use tokio::net::TcpStream;

use chat_rs::*;

static INPUT_ROWS: AtomicU16 = AtomicU16::new(1);

type Messages = Arc<Mutex<Vec<(String, u16)>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let address = env::args()
        .nth(1)
        .unwrap_or_else(|| prompt_msg("Please input the server IP: ").unwrap());

    println!("Connecting to {}:7878", address);

    let mut stream = connect_stream(address).await.unwrap_or_else(|err| {
        eprintln!("Error on connecting: {}", err.to_string());
        process::exit(1);
    });
    let nick = prompt_msg("Enter nickname: ")?;

    let mut buffer = [0u8; MSG_LENGTH];

    stream.send_msg(&Msg::NickChange(nick.clone())).await?;

    match stream.receive_msg(&mut buffer).await {
        Ok(Msg::ConnectionAccepted) => println!("Connected."),
        Ok(Msg::ConnectionEncrypted) => {
            println!("Connected. Encrypting...");
            stream.encrypt().await?;
        }
        Ok(msg) => {
            eprintln!("Server refused connection: {}", msg.string());
            process::exit(0)
        }
        Err(e) => {
            println!("Error connecting to server: {}", e.to_string());
            process::exit(0)
        }
    }

    let messages = Arc::from(Mutex::from(Vec::new()));

    let (reader, writer) = stream.into_split();

    tokio::spawn({
        let messages = messages.clone();
        async move { listen(reader, messages).await }
    });

    handle_input(writer, messages).await?;
    Ok(())
}

async fn connect_stream(address: String) -> Result<ChatStream, io::Error> {
    let stream = TcpStream::connect(format!("{}:7878", address)).await?;
    Ok(ChatStream::new(stream))
}

async fn listen(mut reader: ChatReaderHalf, messages: Messages) {
    let mut buffer = [0u8; MSG_LENGTH];
    let mut stdout = io::stdout();
    loop {
        let msg = match reader.receive_msg(&mut buffer).await {
            Err(_) => {
                execute!(stdout, terminal::LeaveAlternateScreen).unwrap();
                terminal::disable_raw_mode().unwrap();
                println!("Disconnected from server.");
                process::exit(0);
            }
            Ok(msg) => msg,
        };

        add_message(msg, &messages);
        draw_messages(&messages, &mut stdout).unwrap();
    }
}

/// Adds a message to the messages vector while keeping it small by removing old messages.
fn add_message(msg: Msg, messages: &Messages) {
    let mut messages = messages.lock().unwrap();
    let string = stringify_message(msg);
    let lines = get_line_amount(&string);

    messages.push((string, lines));

    let (_, y) = terminal::size().unwrap();
    let maxlen = 2 * (y - INPUT_ROWS.load(Ordering::SeqCst)); // x2 so that messages behave better on-screen

    if messages.len() > maxlen.into() {
        let upper = messages.len() - (maxlen as usize);
        messages.drain(0..upper);
    }
}

fn stringify_message(msg: Msg) -> String {
    use Attribute::Bold;
    use Msg::*;
    match msg {
        NickedUserMsg(nick, message) => format!("{}> {}", nick.red().attribute(Bold), message),
        NickedNickChange(prev, curr) => format!(
            "! {} has changed their nickname to {}",
            prev.red().attribute(Bold),
            curr.red().attribute(Bold)
        ),

        NickedConnect(nick) => format!("! {} has joined the chat.", nick.red().attribute(Bold)),
        NickedDisconnect(nick) => format!("! {} has left the chat.", nick.red().attribute(Bold)),

        NickedCommand(nick, command) => format!(
            "! {} executed {} (to be implemented properly with the command system)",
            nick.red().attribute(Bold),
            command
        ),

        _ => "???? (this shouldn't have been received by the client!)"
            .blue()
            .to_string(),
    }
}

fn get_line_amount(string: &str) -> u16 {
    let (x, _) = terminal::size().unwrap();
    let mut output = 0;
    for line in string.lines() {
        let chars = line.chars().count() as u16;
        output += 1 + (chars / x);
    }
    output
}

fn draw_messages(messages: &Messages, stdout: &mut io::Stdout) -> Result<(), Box<dyn Error>> {
    let messages = messages.lock().unwrap();
    let (_, y) = terminal::size()?;
    let allowed_rows = y - INPUT_ROWS.load(Ordering::SeqCst) - 1;
    let fits = {
        let mut count = 0;
        let mut lines = 0;
        messages.iter().rev().for_each(|e| {
            lines += e.1;
            if lines <= allowed_rows {
                count += 1;
            }
        });
        count
    };

    let to_print = &messages[(messages.len() - fits)..];
    queue!(
        stdout,
        cursor::SavePosition,
        cursor::MoveTo(0, allowed_rows),
        terminal::Clear(ClearType::FromCursorUp),
        cursor::MoveTo(0, 0)
    )?;
    for tuple in to_print {
        let string = &tuple.0;
        queue!(stdout, style::Print(string), cursor::MoveToNextLine(1))?;
    }
    queue!(stdout, cursor::RestorePosition)?;
    stdout.flush()?;

    Ok(())
}

async fn handle_input(
    mut writer: ChatWriterHalf,
    messages: Messages,
) -> Result<(), Box<dyn Error>> {
    let mut stdout = io::stdout();

    terminal::enable_raw_mode()?;
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        cursor::MoveTo(0, terminal::size()?.1)
    )?;

    let mut string = String::new();
    loop {
        let event = event::read()?;
        if let Event::Key(event) = event {
            let do_break =
                handle_key_event(event, &mut string, &mut writer, &mut stdout, &messages).await?;

            if do_break {
                break;
            }
        } else if let Event::Resize(_, _) = event {
            draw_messages(&messages, &mut stdout)?;
        }
    }

    execute!(stdout, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    Ok(())
}

async fn handle_key_event(
    event: event::KeyEvent,
    string: &mut String,
    writer: &mut ChatWriterHalf,
    stdout: &mut io::Stdout,
    messages: &Messages,
) -> Result<bool, Box<dyn Error>> {
    let (x, y) = terminal::size().unwrap();

    if event.modifiers.contains(KeyModifiers::CONTROL) && event.code == KeyCode::Char('c') {
        return Ok(true);
    } else if event.code == KeyCode::Enter {
        if string.len() > 0 {
            writer.send_msg(&Msg::UserMsg(string.clone())).await?;
            string.clear();
            queue!(stdout, terminal::Clear(ClearType::FromCursorUp))?;
        }
        draw_messages(messages, stdout)?;
        INPUT_ROWS.store(1, Ordering::SeqCst);
        execute!(stdout, cursor::MoveTo(0, y))?;
    } else if event.code == KeyCode::Backspace && string.len() > 0 {
        string.pop();
        let (posx, posy) = cursor::position()?;
        if posx == 0 {
            execute!(
                stdout,
                cursor::MoveTo(x - 1, posy - 1),
                style::Print(' '),
                terminal::ScrollDown(1),
                cursor::MoveTo(x - 1, posy)
            )?;
            INPUT_ROWS.fetch_sub(1, Ordering::SeqCst);
            draw_messages(&messages, stdout)?;
        } else {
            execute!(
                stdout,
                cursor::MoveLeft(1),
                style::Print(' '),
                cursor::MoveLeft(1)
            )?;
        }
    } else if let KeyCode::Char(c) = event.code {
        if !event.modifiers.contains(KeyModifiers::CONTROL) {
            string.push(c);
            execute!(stdout, style::Print(c))?;
            let (posx, _) = cursor::position()?;
            if posx == 1 && string.len() != 1 {
                INPUT_ROWS.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
    Ok(false)
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
