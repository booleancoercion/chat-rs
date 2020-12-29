use std::sync::{Arc, Mutex};

use anyhow::Result;
use iced::{
    Element, Text, Color, Align, Length, Container, Row, Column
};

use chat_rs::*;
use crate::style;

#[derive(Debug, Clone)]
pub enum AppMessage {
    AddressChanged(String),
    NickChanged(String),
    ButtonPressed,
    Connected(Arc<Mutex<Option<ChatStream>>>),
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

pub fn visualise_msg(msg: &Msg) -> Element<'static, AppMessage> {
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