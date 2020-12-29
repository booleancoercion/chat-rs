use iced_futures::futures;
use std::hash::Hash;
use std::time::Instant;
use std::sync::Arc;

use tokio::sync::Mutex;

use chat_rs::*;

pub struct Listen {
    unique: Instant,
    reader: Arc<Mutex<ChatReaderHalf>>,
}

impl Listen {
    pub fn new(reader: ChatReaderHalf) -> Self {
        Self {
            // TODO: Find a more reliably unique value
            unique: Instant::now(),
            reader: Arc::new(Mutex::new(reader))
        }
    }

    pub fn sub(&self) -> iced::Subscription<Msg>
    {
        ListenSubscription::sub(self.reader.clone(), self.unique)
    }
}

pub struct ListenSubscription {
    unique: Instant,
    reader: Arc<Mutex<ChatReaderHalf>>
}

impl ListenSubscription {
    pub fn sub(reader: Arc<Mutex<ChatReaderHalf>>, unique: Instant) -> iced::Subscription<Msg> {
        iced::Subscription::from_recipe(Self {
            unique,
            reader
        })
    }
}

impl<H, I> iced_futures::subscription::Recipe<H, I> for ListenSubscription
where
    H: std::hash::Hasher,
{
    type Output = Msg;

    fn hash(&self, state: &mut H) {
        self.unique.hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: futures::stream::BoxStream<'static, I>,
    ) -> futures::stream::BoxStream<'static, Self::Output> {
        let mut buffer = [0u8; MSG_LENGTH];
        Box::pin(
            futures::stream::unfold(self.reader.clone(), move |reader| async move {
                let mut guard = reader.lock().await;
                if let Ok(msg) = guard.receive_msg(&mut buffer).await {
                    drop(guard);
                    Some((msg, reader))
                } else {
                    let _: () = iced::futures::future::pending().await;

                    None
                }
            })
        )
    }
}
