use serde::{Serialize, Deserialize};
use solutions::{message::Envelope, io::io_channel};
use tokio::sync::mpsc::UnboundedSender;
use tracing_subscriber::EnvFilter;
use std::{collections::HashMap, sync::atomic::{AtomicUsize, Ordering}};


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
    Broadcast {
        message: usize,
    },
    BroadcastOk,
    Read,
    ReadOk {
        messages: Vec<usize>
    },
    Topology {
        topology: HashMap<String, Vec<String>>
    },
    TopologyOk
}

fn message_id() -> usize {
    MSG_ID.fetch_add(1, Ordering::SeqCst)
}

#[derive(Debug, Clone, Default)]
pub struct State {
    my_id: String,
    topology: HashMap<String, Vec<String>>,
    seen_messages: Vec<usize>,
}


#[tracing::instrument(skip(writer))]
pub async fn handle_envelope(
    state: &mut State,
    envelope: Envelope<Payload>, 
    writer: UnboundedSender<Envelope<Payload>>
) {
    match &envelope.body.message {
        Payload::Init { node_id, .. } => {

            state.my_id = node_id.clone();

            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::InitOk
            );
            writer.send(reply).unwrap();
        },
        Payload::Topology { topology } => {
            state.topology = topology.clone();
            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::TopologyOk
            );
            writer.send(reply).unwrap();
        },
        Payload::Broadcast { message } => {
            state.seen_messages.push(*message);
            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::BroadcastOk
            );
            writer.send(reply).unwrap();
        },
        Payload::Read => {
            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::ReadOk { messages: state.seen_messages.clone() }
            );
            writer.send(reply).unwrap();
        }

        _ => {}
    }
}

pub async fn server() {
    let mut state = State::default();
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
