use serde::{Serialize, Deserialize};
use solutions::{io::io_channel, message::{Body, Envelope}};
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;
use tracing_subscriber::EnvFilter;
use std::{collections::HashMap, sync::atomic::{AtomicUsize, Ordering}, time::Duration};
use std::sync::{Arc, Mutex};


static MSG_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    TopologyOk,
    Gossip {
        messages: Vec<usize>,
    },
    GossipOk,
}

fn message_id() -> usize {
    MSG_ID.fetch_add(1, Ordering::SeqCst)
}

#[derive(Debug, Clone, Default)]
pub struct State {
    my_id: String,
    topology: HashMap<String, Vec<String>>,
    seen_messages: Vec<usize>,
    // messages remaining to gossip, by neighbor.
    buffered_messages: HashMap<String, Vec<usize>>,
}


impl State {
    pub fn neighbors(&self) -> impl Iterator<Item=&String> + '_ {
        // Assume we can talk to everyone rn.
        self.topology.keys().filter(|&key| key != &self.my_id)
    }
}


#[tracing::instrument(skip(writer))]
pub async fn handle_envelope(
    state: Arc<Mutex<State>>,
    envelope: Envelope<Payload>, 
    writer: UnboundedSender<Envelope<Payload>>
) {
    match &envelope.body.message {
        Payload::Init { node_id, .. } => {
            let mut state = state.lock().unwrap();
            state.my_id = node_id.clone();

            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::InitOk
            );
            writer.send(reply).unwrap();
        },
        Payload::Topology { topology } => {
            let mut state = state.lock().unwrap();
            state.topology = topology.clone();
            
            let neighbors = state.neighbors().cloned().collect::<Vec<_>>();
            for neighbor in neighbors {
                state
                .buffered_messages
                .entry(neighbor)
                .or_default();
            }

            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::TopologyOk
            );
            writer.send(reply).unwrap();
        },
        Payload::Broadcast { message } => {
            let mut state = state.lock().unwrap();
            state.seen_messages.push(*message);

            let neighbors = state.neighbors().cloned().collect::<Vec<_>>();
            for neighbor in neighbors {
                state
                .buffered_messages
                .entry(neighbor.to_owned())
                .and_modify(|messages| messages.push(*message));
            }

            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::BroadcastOk
            );
            writer.send(reply).unwrap();
        },
        Payload::Read => {
            let state = state.lock().unwrap();
            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::ReadOk { messages: state.seen_messages.clone() }
            );
            writer.send(reply).unwrap();
        },
        Payload::Gossip { messages } => {
            let mut state = state.lock().unwrap();
            state.seen_messages.extend(messages);

            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::GossipOk
            );
            writer.send(reply).unwrap();
        },
        Payload::GossipOk => {
            // clear the buffered messages for this neighbor
            // once we get an ack.
            let node = envelope.source.clone();
            let mut state = state.lock().unwrap();
            state.buffered_messages.get_mut(&node).unwrap().clear();
            debug!(node = node, "cleared buffered messages for node");
        }

        _ => {}
    }
}


#[tracing::instrument(skip(writer))]
pub async fn gossip_every_so_often(
    state: Arc<Mutex<State>>, 
    writer: UnboundedSender<Envelope<Payload>>
) {
    let mut interval = tokio::time::interval(Duration::from_millis(1000));
    interval.tick().await;

    loop {
        interval.tick().await;
        {
            let state = state.lock().unwrap();
            for neighbor in state.neighbors() {
                let buffered_messages = state.buffered_messages.get(neighbor).unwrap();
                let payload = Payload::Gossip { messages: buffered_messages.clone() };
                let body = Body { msg_id: Some(message_id()), in_reply_to: None, message: payload };
                let envelope = Envelope::new(&state.my_id, neighbor, body.clone());
                writer.send(envelope).unwrap();
            }
        }    
    }
}

pub async fn server() {
    let state = Arc::new(Mutex::new(State::default()));
    let (writer, mut reader, _) = io_channel::<Envelope<Payload>>();

    let state_cp = state.clone();
    let writer_cp = writer.clone();

    tokio::task::spawn(gossip_every_so_often(state_cp, writer_cp));

    while let Some(envelope) = reader.recv().await {
        handle_envelope(state.clone(), envelope, writer.clone()).await;
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
