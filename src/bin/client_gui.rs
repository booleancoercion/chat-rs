use std::net::TcpStream;
use std::process;

use iced::{Element, Sandbox, Settings, Text, Column, button, Button,
           text_input, TextInput, Length, HorizontalAlignment, Container,
           Align};

use chat_rs::{ChatStream, Msg, MSG_LENGTH};

pub fn main() -> iced::Result {
    ChatClient::run(Settings::default())
}

enum ChatClient {
    Login(LoginState),
    Ready {
        messages: Vec<Msg>,
        stream: ChatStream,
        peer_addr: std::net::SocketAddr,
        state: ReadyState
    }
}

#[derive(Debug, Default)]
struct LoginState {
    text_addr: text_input::State,
    text_addr_val: String,

    text_nick: text_input::State,
    text_nick_val: String,

    login_button: button::State
}

#[derive(Debug, Default)]
struct ReadyState {

}

impl Sandbox for ChatClient {
    type Message = AppMessage;

    fn new() -> ChatClient {
        ChatClient::Login(LoginState::default())
    }
    fn title(&self) -> String {
        format!("chat-rs{}", if let ChatClient::Ready { messages: _, stream: _, peer_addr, state: _ } = self {
            String::from(": ") + &peer_addr.to_string()
        } else {
            "".to_string()
        })
    }

    fn update(&mut self, message: Self::Message) {
        match self {
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
                        let stream = TcpStream::connect(format!("{}:7878", text_addr_val)).unwrap();
                        let mut stream = ChatStream::new(stream);

                        let mut buffer = [0u8; MSG_LENGTH];
    
                        stream.send_data(&Msg::NickChange(text_nick_val.clone())).unwrap();

                        match stream.receive_data(&mut buffer) {
                            Ok(Msg::ConnectionAccepted) => println!("Connected."),
                            Ok(Msg::ConnectionEncrypted) => {
                                println!("Connected. Encrypting...");
                                stream.encrypt().unwrap();
                            },
                            Ok(msg) => {
                                eprintln!("Server refused connection: {}", msg.string());
                                process::exit(0)
                            },
                            Err(e) => {
                                println!("Error connecting to server: {}", e.to_string());
                                process::exit(0)
                            }
                        }
                        
                        let peer_addr = stream.peer_addr().unwrap();

                        *self = ChatClient::Ready {
                            messages: vec![],
                            stream,
                            peer_addr,
                            state: ReadyState::default()
                        }
                    }
                }
            },

            _ => todo!()
        }
    }

    fn view(&mut self) -> Element<Self::Message> {
        match self {
            ChatClient::Login(LoginState {
                text_addr,
                text_addr_val,
                text_nick,
                text_nick_val,
                login_button
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
                    AppMessage::AddressChanged
                )
                .padding(15)
                .size(30);

                let nick_input = TextInput::new(
                    text_nick,
                    "Enter your nickname",
                    text_nick_val,
                    AppMessage::NickChanged
                )
                .padding(15)
                .size(30);

                let button = Button::new(
                    login_button,
                    Text::new("Connect").size(30)
                )
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
                
                Container::new(content).width(Length::Fill).center_x().center_y().into()
            },

            _ => todo!()
        }
    }
}

#[derive(Debug, Clone)]
enum AppMessage {
    AddressChanged(String),
    NickChanged(String),
    ButtonPressed
}

mod style {
    use iced::{button, Background, Color, Vector};

    pub enum Button {
        Simple
    }

    impl button::StyleSheet for Button {
        fn active(&self) -> button::Style {
            match self {
                Button::Simple => {
                    button::Style {
                        background: Some(Background::Color(
                            Color::from_rgb(0.2, 0.2, 0.7),
                        )),
                        border_radius: 10.0,
                        text_color: Color::WHITE,
                        ..button::Style::default()
                    }
                }
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
}