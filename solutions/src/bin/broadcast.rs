use serde::{Serialize, Deserialize};
use solutions::{io::io_channel, message::{Body, Envelope}};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, trace};
use tracing_subscriber::EnvFilter;
use std::{collections::{HashMap, HashSet}, sync::atomic::{AtomicUsize, Ordering}, time::Duration};
use std::sync::{Arc, Mutex};
use clap::Parser;

#[derive(Debug, Parser)]
#[clap(author, version)]
pub struct Opts {
    #[clap(short, long, help = "choose 1 out of every STRIDE nodes as a direct neighbor", env = "STRIDE")]
    pub stride: usize,
    #[clap(short, long, help = "Number of milliseconds to wait before attempting to sync unacknowledged messages.", env = "TICK_RATE_MS")]
    pub tick_rate_ms: u64
}


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
    Sync {
        messages: Vec<usize>,
    },
    SyncOk {
        messages: Vec<usize>,
    }
}

fn message_id() -> usize {
    MSG_ID.fetch_add(1, Ordering::SeqCst)
}


#[derive(Debug, Clone, Default)]
pub struct RemoteNode {
    pub node_id: String,
    pub unacknowledged_messages: Vec<usize>
}

impl RemoteNode {
    pub fn send_message(&mut self, message: usize) {
        self.unacknowledged_messages.push(message);
    }

    pub fn acknowledge_synced(&mut self, messages: &[usize]) {
        self.unacknowledged_messages.retain(|message| !messages.contains(message));
        trace!(node_id = self.node_id, "remote node has acknowledged these messages: {:?}", messages);
    }

    pub fn has_unacknowledged_messages(&self) -> bool {
        !self.unacknowledged_messages.is_empty()
    }
}


#[derive(Debug, Clone, Default)]
pub struct State {
    my_id: String,
    all_node_ids: Vec<String>,
    neighbors: Vec<String>,
    nodes: HashMap<String, RemoteNode>,
    messages: HashSet<usize>,
    stride: usize,
    tick_rate: Duration
}


impl State {
    pub fn seen_messages(&self) -> Vec<usize> {
        self.messages.iter().copied().collect()
    }
}


#[tracing::instrument(skip(writer))]
pub async fn handle_envelope(
    state: Arc<Mutex<State>>,
    envelope: Envelope<Payload>, 
    writer: UnboundedSender<Envelope<Payload>>
) {
    match &envelope.body.message {
        Payload::Init { node_id, node_ids } => {
            let mut state = state.lock().unwrap();
            state.my_id = node_id.clone();
            state.all_node_ids = node_ids.clone();

            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::InitOk
            );
            writer.send(reply).unwrap();
        },
        Payload::Topology { .. } => {
            let mut state = state.lock().unwrap();

            let our_position = 
                state.all_node_ids
                .iter()
                .position(|node_id| node_id == &state.my_id)
                .unwrap();

            state.neighbors = 
                state
                .all_node_ids
                .iter()
                .skip((our_position + 1) % state.stride)
                .step_by(state.stride)
                .cloned()
                .collect();

            for neighbor in &state.neighbors.clone() {
                state.nodes.insert(neighbor.clone(), Default::default());
            }

            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::TopologyOk
            );
            writer.send(reply).unwrap();
        },
        Payload::Broadcast { message } => {
            let mut state = state.lock().unwrap();
            let inserted = state.messages.insert(*message);
            let neighbors = state.neighbors.clone();

            // if we saw it the first time, we should try to tell others about it later.
            if inserted {
                for neighbor in neighbors {
                    state
                    .nodes
                    .get_mut(&neighbor)
                    .unwrap()
                    .send_message(*message);
                }
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
                Payload::ReadOk { messages: state.seen_messages() }
            );
            writer.send(reply).unwrap();
        },
        Payload::Sync { messages: inbound } => {
            let mut state = state.lock().unwrap();
            for &message in inbound {
                if state.messages.insert(message) {
                    for neighbor in state.neighbors.clone() {
                        state.nodes.get_mut(&neighbor).unwrap().send_message(message);
                    }
                }
            }
            let reply = envelope.reply_with(Some(message_id()), Payload::SyncOk { messages: inbound.clone() });
            writer.send(reply).unwrap();
        },
        Payload::SyncOk { messages: acknowledged_messages } => {
            // Update our knowledge that this specific node
            // has acknowledged our messages.
            let mut state = state.lock().unwrap();
            let neighbor = envelope.source.clone();
            state.nodes.get_mut(&neighbor).unwrap().acknowledge_synced(acknowledged_messages);
            debug!(node = neighbor, "cleared buffered messages for node");
        }

        _ => {}
    }
}


#[tracing::instrument(skip(writer))]
pub async fn gossip_every_so_often(
    state: Arc<Mutex<State>>,
    writer: UnboundedSender<Envelope<Payload>>
) {
    let mut interval = tokio::time::interval(state.lock().unwrap().tick_rate);
    interval.tick().await;

    loop {
        interval.tick().await;
        {
            let mut state = state.lock().unwrap();
            let my_id = state.my_id.clone();
            for (neighbor, node) in state.nodes.iter_mut() {
                if node.has_unacknowledged_messages() {
                    let envelope = Envelope::new(
                        &my_id, 
                        neighbor, 
                        Body { 
                            msg_id: Some(message_id()), 
                            in_reply_to: None, 
                            message: Payload::Sync { 
                                messages: node.unacknowledged_messages.to_vec()
                            }
                        }
                    );
                    writer.send(envelope).unwrap();
                }
            }
        }    
    }
}

pub async fn server(opts: Opts) {
    let state = Arc::new(Mutex::new(State::default()));
    {
        let mut guard = state.lock().unwrap();
        guard.tick_rate = Duration::from_millis(opts.tick_rate_ms);
        guard.stride = opts.stride;
    }
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
    let opts = Opts::parse();

    // For 3d) STRIDE=3 TICK_RATE_MS=155
    // For 3e) STRIDE=4 TICK_RATE_MS=250

    tracing_subscriber::FmtSubscriber::builder()
    .with_writer(std::io::stderr)
    // .pretty()
    .with_ansi(false)
    .with_env_filter(EnvFilter::from_default_env())
    .init();

    debug!(opts = ?opts, "starting server...");
    server(opts).await;
}
