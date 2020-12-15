use iced::{Element, Sandbox, Settings, Text};

use chat_rs::{ChatStream, Msg};

pub fn main() -> iced::Result {
    ChatClient::run(Settings::default())
}

struct ChatClient {
    messages: Vec<Msg>
}

impl Sandbox for ChatClient {
    type Message = ();

    fn new() -> ChatClient {
        ChatClient {
            messages: vec![]
        }
    }

    fn title(&self) -> String {
        String::from("Resizing this is laggy AF")
    }

    fn update(&mut self, _message: Self::Message) {
        // This application has no interactions
    }

    fn view(&mut self) -> Element<Self::Message> {
        Text::new("Hello, world!").into()
    }
}