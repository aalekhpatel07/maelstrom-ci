use serde::{Serialize, Deserialize};
use solutions::{message::Envelope, io::io_channel};
use tokio::sync::mpsc::UnboundedSender;
use tracing_subscriber::EnvFilter;
use std::sync::atomic::{AtomicUsize, Ordering};
use rand::Rng;

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
    Generate,
    GenerateOk {
        id: String
    }
}

fn message_id() -> usize {
    MSG_ID.fetch_add(1, Ordering::SeqCst)
}

#[derive(Debug, Clone)]
pub struct State {
    id: String
}


#[tracing::instrument(skip(writer))]
pub async fn handle_envelope(
    state: &mut State,
    envelope: Envelope<Payload>, 
    writer: UnboundedSender<Envelope<Payload>>
) {
    match &envelope.body.message {
        Payload::Generate => {
            let msg_id = message_id();
            let id = format!("{}_{}", state.id, msg_id);

            let reply = envelope.reply_with(
                Some(msg_id),
                Payload::GenerateOk { id }
            );
            writer.send(reply).unwrap();
        },
        Payload::Init { node_id, .. } => {

            let mut rng = rand::thread_rng();
            let offset = rng.gen::<usize>();
            state.id = format!("{}_{}", node_id, offset);

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
    let mut state = State { id: "".to_owned() };
    let (writer, mut reader, _) = io_channel::<Envelope<Payload>>();
    while let Some(envelope) = reader.recv().await {
        handle_envelope(&mut state, envelope, writer.clone()).await;
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
