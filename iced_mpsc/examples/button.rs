use iced::*;
use iced_mpsc::Mpsc;

fn main() {
    App::run(Default::default())
}

struct App {
    clicky: button::State,
    n_clicks: usize,
    sender: Option<iced_mpsc::Sender<()>>,
    iced_mpsc: Mpsc<()>,
}

#[derive(Debug, Clone)]
enum Message {
    Clicky,
    Mpsc(iced_mpsc::Message<()>),
}

impl Application for App {
    type Executor = executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        let instance = Self {
            clicky: button::State::new(),
            sender: None,
            iced_mpsc: Mpsc::new(80),
            n_clicks: 0,
        };
        (instance, Command::none())
    }

    fn title(&self) -> String {
        "MPSC Demonstration".into()
    }

    fn view(&mut self) -> Element<Self::Message> {
        Button::new(
            &mut self.clicky,
            Text::new(format!("{} Clicks", self.n_clicks)),
        )
        .on_press(Message::Clicky)
        .into()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Mpsc(iced_mpsc::Message::Received(())) => self.n_clicks += 1,
            Message::Mpsc(iced_mpsc::Message::Sender(mut tx)) => {
                self.sender = Some(tx.clone());
                std::thread::spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    tx.try_send(()).unwrap();
                });
            }
            Message::Clicky => {
                if let Some(tx) = &mut self.sender {
                    tx.try_send(()).expect("Sender vanished!")
                } else {
                    panic!("Sender not set yet!")
                }
            }
        }
        Command::none()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        self.iced_mpsc.sub().map(Message::Mpsc)
    }
}
