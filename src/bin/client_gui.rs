#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

use anyhow::{bail, Result};
use iced::{
    button, executor, scrollable, text_input, Align, Application, Button, Color, Column, Command,
    Container, Element, HorizontalAlignment, Length, Row, Scrollable, Settings, Subscription, Text,
    TextInput,
};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use chat_rs::*;
use iced_mpsc::Mpsc;

pub fn main() -> iced::Result {
    ChatClient::run(Settings::default())
}

enum ChatClient {
    Error(String),
    Login(LoginState),
    Connecting(Option<(ChatStream, Mpsc<Msg>)>),
    Ready {
        messages: Vec<Msg>,
        msg_mpsc: iced_mpsc::Mpsc<Msg>,
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

                        *self = ChatClient::Connecting(None);
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

            ChatClient::Connecting(opt) => match message {
                AppMessage::Connected(stream) => {
                    let stream = stream.lock().unwrap().take().unwrap();
                    let mpsc = Mpsc::new(32);
                    opt.replace((stream, mpsc));
                }

                AppMessage::Sender(mut sender) => {
                    let (stream, mpsc) = opt.take().unwrap();
                    let peer_addr = stream.peer_addr().unwrap();
                    let (mut reader, mut writer) = stream.into_split();

                    let (tx, mut rx) = mpsc::channel::<Msg>(32);

                    *self = ChatClient::Ready {
                        messages: vec![],
                        msg_mpsc: mpsc,
                        writer_channel: tx,
                        peer_addr,
                        state: ReadyState::default(),
                    };

                    tokio::spawn(async move {
                        let mut buffer = [0u8; MSG_LENGTH];
                        while let Ok(msg) = reader.receive_msg(&mut buffer).await {
                            sender.start_send(msg).unwrap();
                        }
                    });

                    tokio::spawn(async move {
                        while let Some(msg) = rx.recv().await {
                            writer.send_msg(&msg).await.unwrap();
                        }
                    });
                }
                _ => {}
            },

            ChatClient::Ready {
                messages,
                writer_channel,
                state,
                ..
            } => {
                match message {
                    AppMessage::ChatMsg(msg) => {
                        messages.push(msg);
                        if !state.scroll.is_scroller_grabbed() {
                            // UGLY: replace when PR lands
                            state.scroll = unsafe {
                                let mut tmp =
                                    std::mem::transmute::<_, (Option<f32>, f32)>(state.scroll);
                                tmp.1 = 999999.0;
                                std::mem::transmute::<_, scrollable::State>(tmp)
                            };
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
                }
            }
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
                    .horizontal_alignment(HorizontalAlignment::Center);

                let error_text = Text::new(s.to_string())
                    .width(Length::Fill)
                    .size(50)
                    .color([1.0, 0.0, 0.0])
                    .horizontal_alignment(HorizontalAlignment::Center);

                let col: Column<AppMessage> = Column::new()
                    .align_items(Align::Center)
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
                    .horizontal_alignment(HorizontalAlignment::Center);

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
                    .align_items(Align::Center);

                Container::new(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y()
                    .into()
            }

            ChatClient::Connecting(_) => {
                let title = Text::new("Connecting...")
                    .width(Length::Fill)
                    .size(100)
                    .color([0.5, 0.5, 0.5])
                    .horizontal_alignment(HorizontalAlignment::Center);

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
                    .align_items(Align::Start)
                    .height(Length::Fill)
                    .width(Length::Fill)
                    .spacing(5);

                for msg in messages {
                    messages_scroll = messages_scroll.push(visualise_msg(msg));
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
                    .align_items(Align::Center)
                    .width(Length::Fill)
                    .height(Length::Shrink)
                    .spacing(10)
                    .push(msg_input)
                    .push(send_button);

                let col = Column::new()
                    .align_items(Align::Center)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .spacing(10)
                    .push(messages_scroll)
                    .push(row);

                Container::new(col)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(15)
                    .align_x(Align::Start)
                    .align_y(Align::Start)
                    .into()
            }
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        match self {
            ChatClient::Ready { msg_mpsc: mpsc, .. } | ChatClient::Connecting(Some((_, mpsc))) => {
                mpsc.sub().map(|message| match message {
                    iced_mpsc::Message::Sender(sender) => AppMessage::Sender(sender),
                    iced_mpsc::Message::Received(msg) => AppMessage::ChatMsg(msg),
                })
            }

            _ => Subscription::none(),
        }
    }
}

#[derive(Debug, Clone)]
enum AppMessage {
    AddressChanged(String),
    NickChanged(String),
    ButtonPressed,
    Connected(Arc<Mutex<Option<ChatStream>>>),
    Sender(iced_mpsc::Sender<Msg>),
    ChatMsg(Msg),
    InputChanged(String),
    Send,
    Sent(()),

    Error(String),
}

impl AppMessage {
    pub fn or_error<T>(
        g: impl (Fn(T) -> AppMessage) + 'static + Send,
    ) -> impl Fn(Result<T>) -> AppMessage + 'static + Send {
        move |r| match r {
            Ok(val) => g(val),
            Err(err) => AppMessage::Error(format!("{}", err)),
        }
    }
}

fn visualise_msg(msg: &Msg) -> Element<'static, AppMessage> {
    use Msg::*;

    match msg {
        NickedUserMsg(nick, message) => {
            let nick_text = Text::new(nick)
                .size(14)
                .color(Color::from_rgb8(248, 47, 58));

            let message_text = Text::new(message).size(14).color(Color::from_rgb8(0, 0, 0));

            let content = Column::new()
                .align_items(Align::Start)
                .height(Length::Shrink)
                .width(Length::Shrink)
                .spacing(10)
                .padding(10)
                .push(nick_text)
                .push(message_text);

            Container::new(content)
                .height(Length::Shrink)
                .width(Length::Shrink)
                .style(style::Container::UserMessage)
                .into()
        }
        NickedNickChange(prev, curr) => {
            let prev_text = Text::new(prev)
                .size(14)
                .color(Color::from_rgb8(248, 47, 58));
            // set font

            let message_text = Text::new(" has changed their nickname to ")
                .size(14)
                .color(Color::from_rgb8(45, 45, 45));
            // set font

            let curr_text = Text::new(curr)
                .size(14)
                .color(Color::from_rgb8(248, 47, 58));
            // set font

            let content = Row::new()
                .align_items(Align::Center)
                .height(Length::Shrink)
                .width(Length::Shrink)
                .spacing(0)
                .padding(10)
                .push(prev_text)
                .push(message_text)
                .push(curr_text);

            Container::new(content)
                .height(Length::Shrink)
                .width(Length::Shrink)
                .style(style::Container::SystemMessage)
                .into()
        }

        NickedConnect(nick) => system_message(nick, " has joined the chat."),
        NickedDisconnect(nick) => system_message(nick, " has left the chat."),

        NickedCommand(nick, command) => {
            system_message(nick, &format!(" executed command: {}", command))
        }

        _ => system_message("ERROR: UNIMPLEMENTED", ""),
    }
}

fn system_message(nick: &str, message: &str) -> Element<'static, AppMessage> {
    let nick_text = Text::new(nick)
        .size(14)
        .color(Color::from_rgb8(248, 47, 58));
    // set font

    let message_text = Text::new(message)
        .size(14)
        .color(Color::from_rgb8(45, 45, 45));
    // set font

    let content = Row::new()
        .align_items(Align::Center)
        .height(Length::Shrink)
        .width(Length::Shrink)
        .spacing(0)
        .padding(10)
        .push(nick_text)
        .push(message_text);

    Container::new(content)
        .height(Length::Shrink)
        .width(Length::Shrink)
        .style(style::Container::SystemMessage)
        .into()
}

mod style {
    use iced::{button, container, Background, Color, Vector};

    pub enum Button {
        Simple,
    }

    impl button::StyleSheet for Button {
        fn active(&self) -> button::Style {
            match self {
                Button::Simple => button::Style {
                    background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.7))),
                    border_radius: 10.0,
                    text_color: Color::WHITE,
                    ..button::Style::default()
                },
            }
        }

        fn hovered(&self) -> button::Style {
            let active = self.active();

            button::Style {
                shadow_offset: active.shadow_offset + Vector::new(2.0, 2.0),
                ..active
            }
        }
    }

    pub enum Container {
        SystemMessage,
        UserMessage,
    }

    impl container::StyleSheet for Container {
        fn style(&self) -> container::Style {
            let color = match self {
                Container::SystemMessage => Color::from_rgb8(199, 243, 239),
                Container::UserMessage => Color::from_rgb8(220, 220, 220),
            };

            container::Style {
                background: Some(Background::Color(color)),
                border_radius: 10.0,
                ..container::Style::default()
            }
        }
    }
}
