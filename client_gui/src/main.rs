#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

use anyhow::bail;
use iced::{
    alignment::{Horizontal, Vertical},
    button, executor, scrollable, text_input, Alignment, Application, Button, Column, Command,
    Container, Element, Length, Row, Scrollable, Settings, Subscription, Text, TextInput,
};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use chat_rs::*;

mod listen;
mod messages;
mod style;

use listen::*;
use messages::AppMessage;

pub fn main() -> iced::Result {
    ChatClient::run(Settings::default())
}

enum ChatClient {
    Error(String),
    Login(LoginState),
    Connecting,
    Ready {
        messages: Vec<Msg>,
        listener: Listen,
        writer_channel: mpsc::Sender<Msg>,
        peer_addr: std::net::SocketAddr,
        state: ReadyState,
    },
}

#[derive(Debug, Default)]
struct LoginState {
    text_addr: text_input::State,
    text_addr_val: String,

    text_nick: text_input::State,
    text_nick_val: String,

    login_button: button::State,
}

#[derive(Debug, Default)]
struct ReadyState {
    scroll: scrollable::State,
    input: text_input::State,
    input_value: String,
    send: button::State,
}

impl Application for ChatClient {
    type Message = AppMessage;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (ChatClient, Command<Self::Message>) {
        (ChatClient::Login(LoginState::default()), Command::none())
    }
    fn title(&self) -> String {
        format!(
            "chat-rs{}",
            if let ChatClient::Ready { peer_addr, .. } = self {
                String::from(": ") + &peer_addr.to_string()
            } else {
                "".to_string()
            }
        )
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        if let AppMessage::Error(e) = &message {
            *self = ChatClient::Error(e.to_string())
        }
        match self {
            ChatClient::Error(_) => {}
            ChatClient::Login(LoginState {
                text_addr_val,
                text_nick_val,
                ..
            }) => {
                use AppMessage::*;
                match message {
                    AddressChanged(s) => *text_addr_val = s,
                    NickChanged(s) => *text_nick_val = s,
                    ButtonPressed => {
                        let address = text_addr_val.clone();
                        let nick = text_nick_val.clone();

                        *self = ChatClient::Connecting;
                        return Command::perform(
                            async move {
                                let stream =
                                    TcpStream::connect(format!("{}:7878", address)).await?;
                                let mut stream = ChatStream::new(stream);

                                let mut buffer = [0u8; MSG_LENGTH];

                                stream.send_msg(&Msg::NickChange(nick)).await?;

                                match stream.receive_msg(&mut buffer).await {
                                    Ok(Msg::ConnectionAccepted) => println!("Connected."),
                                    Ok(Msg::ConnectionEncrypted) => {
                                        println!("Connected. Encrypting...");
                                        stream.encrypt().await?;
                                    }
                                    Ok(msg) => bail!("Server refused connection: {}", msg.string()),
                                    Err(e) => {
                                        bail!("Error connecting to server: {}", e.to_string())
                                    }
                                }

                                Ok(Arc::new(Mutex::new(Some(stream))))
                            },
                            AppMessage::or_error(Connected),
                        );
                    }
                    _ => {}
                }
            }

            ChatClient::Connecting => {
                if let AppMessage::Connected(stream) = message {
                    let stream = stream.lock().unwrap().take().unwrap();
                    let peer_addr = stream.peer_addr().unwrap();

                    let (reader, mut writer) = stream.into_split();
                    let listener = Listen::new(reader);

                    let (tx, mut rx) = mpsc::channel::<Msg>(32);

                    *self = ChatClient::Ready {
                        messages: vec![],
                        listener,
                        writer_channel: tx,
                        peer_addr,
                        state: ReadyState::default(),
                    };

                    tokio::spawn(async move {
                        while let Some(msg) = rx.recv().await {
                            writer.send_msg(&msg).await.unwrap();
                        }
                    });
                }
            }

            ChatClient::Ready {
                messages,
                writer_channel,
                state,
                ..
            } => match message {
                AppMessage::ChatMsg(msg) => {
                    messages.push(msg);
                    if !state.scroll.is_scroller_grabbed() {
                        state.scroll.snap_to(1.0);
                    }
                }

                AppMessage::InputChanged(s) => state.input_value = s,
                AppMessage::Send => {
                    let msg = Msg::UserMsg(state.input_value.drain(..).collect());
                    let channel = writer_channel.clone();
                    return Command::perform(
                        async move {
                            channel.send(msg).await?;

                            Ok(())
                        },
                        AppMessage::or_error(AppMessage::Sent),
                    );
                }

                _ => {}
            },
        }

        Command::none()
    }

    fn view(&mut self) -> Element<Self::Message> {
        match self {
            ChatClient::Error(s) => {
                let title = Text::new("An error has occured:")
                    .width(Length::Fill)
                    .size(100)
                    .color([0.5, 0.5, 0.5])
                    .horizontal_alignment(Horizontal::Center);

                let error_text = Text::new(s.to_string())
                    .width(Length::Fill)
                    .size(50)
                    .color([1.0, 0.0, 0.0])
                    .horizontal_alignment(Horizontal::Center);

                let col: Column<AppMessage> = Column::new()
                    .align_items(Alignment::Center)
                    .width(Length::Fill)
                    .padding(10)
                    .spacing(10)
                    .push(title)
                    .push(error_text);

                Container::new(col)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y()
                    .padding(10)
                    .into()
            }
            ChatClient::Login(LoginState {
                text_addr,
                text_addr_val,
                text_nick,
                text_nick_val,
                login_button,
            }) => {
                let title = Text::new("Login")
                    .width(Length::Fill)
                    .size(100)
                    .color([0.5, 0.5, 0.5])
                    .horizontal_alignment(Horizontal::Center);

                let addr_input = TextInput::new(
                    text_addr,
                    "Enter the chat server address",
                    text_addr_val,
                    AppMessage::AddressChanged,
                )
                .padding(15)
                .size(30);

                let nick_input = TextInput::new(
                    text_nick,
                    "Enter your nickname",
                    text_nick_val,
                    AppMessage::NickChanged,
                )
                .padding(15)
                .size(30);

                let button = Button::new(login_button, Text::new("Connect").size(30))
                    .on_press(AppMessage::ButtonPressed)
                    .padding(15)
                    .style(style::Button::Simple);

                let content = Column::new()
                    .max_width(600)
                    .spacing(20)
                    .padding(20)
                    .push(title)
                    .push(addr_input)
                    .push(nick_input)
                    .push(button)
                    .align_items(Alignment::Center);

                Container::new(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y()
                    .into()
            }

            ChatClient::Connecting => {
                let title = Text::new("Connecting...")
                    .width(Length::Fill)
                    .size(100)
                    .color([0.5, 0.5, 0.5])
                    .horizontal_alignment(Horizontal::Center);

                Container::new(title)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y()
                    .into()
            }

            ChatClient::Ready {
                messages,
                state:
                    ReadyState {
                        scroll,
                        input,
                        input_value,
                        send,
                    },
                ..
            } => {
                let mut messages_scroll = Scrollable::new(scroll)
                    .align_items(Alignment::Start)
                    .height(Length::Fill)
                    .width(Length::Fill)
                    .spacing(5);

                for msg in messages {
                    messages_scroll = messages_scroll.push(messages::visualise_msg(msg));
                }

                let msg_input = TextInput::new(
                    input,
                    "Enter a message",
                    input_value,
                    AppMessage::InputChanged,
                )
                .size(20)
                .padding(15)
                .on_submit(AppMessage::Send);

                let send_button = Button::new(send, Text::new("Send").size(20))
                    .padding(15)
                    .on_press(AppMessage::Send);

                let row = Row::new()
                    .align_items(Alignment::Center)
                    .width(Length::Fill)
                    .height(Length::Shrink)
                    .spacing(10)
                    .push(msg_input)
                    .push(send_button);

                let col = Column::new()
                    .align_items(Alignment::Center)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .spacing(10)
                    .push(messages_scroll)
                    .push(row);

                Container::new(col)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(15)
                    .align_x(Horizontal::Left)
                    .align_y(Vertical::Top)
                    .into()
            }
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        match self {
            ChatClient::Ready { listener, .. } => listener.sub().map(AppMessage::ChatMsg),

            _ => Subscription::none(),
        }
    }
}
