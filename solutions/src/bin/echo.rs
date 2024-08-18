use serde::{Serialize, Deserialize};
use solutions::message::Message;
use solutions::io::io_channel;
use std::sync::atomic::{AtomicUsize, Ordering};

static MSG_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Body {
    Echo {
        msg_id: usize,
        echo: String,
    },
    EchoOk {
        msg_id: usize,
        in_reply_to: usize,
        echo: String
    }
}


pub async fn server() {
    let (writer, mut reader, _) = io_channel::<Message<Body>>();
    while let Some(message) = reader.recv().await {
        if let Body::Echo { msg_id, echo } = &message.body {
            writer.send(message.reply_with(
                Body::EchoOk { msg_id: MSG_ID.fetch_add(1, Ordering::SeqCst), in_reply_to: *msg_id, echo: echo.clone() }
            )).unwrap();
        }
    }
}


#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    server().await;
}
