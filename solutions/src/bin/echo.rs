use serde::{Serialize, Deserialize};
use solutions::{message::Envelope, io::io_channel};
use tokio::sync::mpsc::UnboundedSender;
use tracing_subscriber::EnvFilter;
use std::sync::atomic::{AtomicUsize, Ordering};

static MSG_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Payload {
    Init { 
        node_id: String,
        node_ids: Vec<String>,
    },
    InitOk,
    Echo {
        echo: String,
    },
    EchoOk {
        echo: String
    }
}

fn message_id() -> usize {
    MSG_ID.fetch_add(1, Ordering::SeqCst)
}


#[tracing::instrument(skip(writer))]
pub async fn handle_envelope(envelope: Envelope<Payload>, writer: UnboundedSender<Envelope<Payload>>) {
    match &envelope.body.message {
        Payload::Echo { echo } => {
            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::EchoOk { echo: echo.clone() }
            );
            writer.send(reply).unwrap();
        },
        Payload::Init { .. } => {
            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::InitOk
            );
            writer.send(reply).unwrap();
        },
        _ => {}
    }
}

pub async fn server() {
    let (writer, mut reader, _) = io_channel::<Envelope<Payload>>();
    while let Some(envelope) = reader.recv().await {
        handle_envelope(envelope, writer.clone()).await;
    }
}


#[tokio::main]
async fn main() {
    tracing_subscriber::FmtSubscriber::builder()
    .with_writer(std::io::stderr)
    // .pretty()
    .with_ansi(false)
    .with_env_filter(EnvFilter::from_default_env())
    .init();

    server().await;
}
