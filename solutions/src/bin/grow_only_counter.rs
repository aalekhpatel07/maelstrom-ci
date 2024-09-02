use serde::{Serialize, Deserialize};
use solutions::{io::io_channel, message::{Body, Envelope}};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error};
use tracing_subscriber::EnvFilter;
use std::{collections::HashMap, sync::atomic::{AtomicUsize, Ordering}, time::Duration};
use std::sync::{Arc, Mutex};
use clap::Parser;


#[derive(Debug, Parser)]
#[clap(author, version)]
pub struct Opts {
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
    Topology {
        topology: HashMap<String, Vec<String>>
    },
    TopologyOk,

    Read {
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<String>,
    },
    ReadOk {
        value: usize
    },
    Write {
        key: String,
        value: usize
    },
    WriteOk,
    Cas {
        key: String,
        from: usize,
        to: usize,
        create_if_not_exists: Option<bool>
    },
    CasOk,
    Add {
        delta: usize,
    },
    UpdateCounter {
        value: usize,
    },
    AddOk,
    Error {
        code: usize,
        text: String
    }
}

impl TryFrom<serde_json::Value> for Payload {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        let serialized = serde_json::to_vec(&value)?;
        serde_json::from_slice::<Self>(&serialized)
    }
}


fn message_id() -> usize {
    MSG_ID.fetch_add(1, Ordering::SeqCst)
}


#[derive(Debug, Default)]
pub struct State {
    my_id: String,
    all_node_ids: Vec<String>,
    neighbors: Vec<String>,
    uncommitted_total: usize,
    last_known_committed_total: usize,
    cas_deltas: HashMap<usize, usize>,
    // messages: HashSet<usize>,
    tick_rate: Duration,
}


impl State {
    pub fn new() -> Self {
        Default::default()
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
        Payload::Topology { topology } => {
            let mut state = state.lock().unwrap();

            state.neighbors = topology.get(&state.my_id).unwrap().clone();

            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::TopologyOk
            );
            writer.send(reply).unwrap();
        },
        Payload::Add { delta } => {
            let mut state = state.lock().unwrap();
            state.uncommitted_total += delta;

            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::AddOk
            );
            writer.send(reply).unwrap();
        },
        Payload::Read { key } => {
            assert!(key.is_none(), "Clients should not send us read payloads.");
            let state = state.lock().unwrap();
            
            let reply = envelope.reply_with(
                Some(message_id()),
                Payload::ReadOk { value: state.last_known_committed_total }
            );
            writer.send(reply).unwrap();
        },
        Payload::CasOk => {
            // our most recent commit was successful, so we can clear any uncommitted state.
            let mut state = state.lock().unwrap();
            let committed_delta = *state.cas_deltas.get(&envelope.body.in_reply_to.unwrap()).unwrap();
            if committed_delta <= state.uncommitted_total {
                state.uncommitted_total -= committed_delta;
            } else {
                state.uncommitted_total = 0;
            }
            state.last_known_committed_total += committed_delta;

            // // Tell all neighbors about this update, in case they're outta date.
            for neighbor in state.all_node_ids.iter().filter(|&node_id| node_id != &state.my_id) {
                let envelope = Envelope::new(
                    &state.my_id,
                    neighbor,
                    Body {
                        msg_id: Some(message_id()),
                        in_reply_to: None,
                        message: Payload::UpdateCounter { 
                            value: state.last_known_committed_total,
                        }
                    }
                );
                writer.send(envelope).unwrap();
            }
        },
        Payload::Error { code, text } => {
            error!("KVError: [{code}] {text}");
            // We couldn't commit updates. so we gotta sync our last known committed state by issuing a read.
            let state = state.lock().unwrap();

            let envelope = Envelope::new(
                &state.my_id,
                "seq-kv",
                Body {
                    msg_id: Some(message_id()),
                    in_reply_to: None,
                    message: Payload::Read { 
                        key: Some("counter".to_string()), 
                    }
                }
            );
            writer.send(envelope).unwrap();
        },
        Payload::ReadOk { value } => {
            debug!("KVReadOk: {value}");
            let mut state = state.lock().unwrap();
            if *value >= state.last_known_committed_total {
                state.last_known_committed_total = *value;
            }
        },
        Payload::UpdateCounter { value } => {
            debug!("UpdateCounter: {value}");
            let mut state = state.lock().unwrap();
            if *value >= state.last_known_committed_total {
                state.last_known_committed_total = *value;
            }
        }
        _ => {}
    }
}


#[tracing::instrument(skip(writer))]
pub async fn commit_buffered_delta_every_so_often(
    state: Arc<Mutex<State>>,
    writer: UnboundedSender<Envelope<Payload>>
) {
    let tick_rate = state.lock().unwrap().tick_rate;

    let mut interval = tokio::time::interval(tick_rate);
    interval.tick().await;

    loop {
        interval.tick().await;
        {
            let mut state = state.lock().unwrap();
            let my_id = state.my_id.clone();
            if state.uncommitted_total > 0 {
                // Try to commit unbuffered counter updates to a last known committed value.

                // So the thing with seq-kv's is that an acknowledged 
                // commit from a node X is not necessarily reflected in a commit 
                // from a node Y.
                let envelope = Envelope::new(
                    &my_id,
                    "seq-kv",
                    Body {
                        msg_id: Some(message_id()),
                        in_reply_to: None,
                        message: Payload::Cas { 
                            key: "counter".to_string(), 
                            from: state.last_known_committed_total, 
                            to: (state.last_known_committed_total + state.uncommitted_total), 
                            create_if_not_exists: Some(true)
                        }
                    }
                );
                let cas_delta = state.uncommitted_total;
                state.cas_deltas.insert(envelope.msg_id().unwrap(), cas_delta);
                writer.send(envelope).unwrap();

            }
            // Ask for the most recent committed value.
            let envelope = Envelope::new(
                &state.my_id,
                "seq-kv",
                Body {
                    msg_id: Some(message_id()),
                    in_reply_to: None,
                    message: Payload::Read { 
                        key: Some("counter".to_string()), 
                    }
                }
            );
            writer.send(envelope).unwrap();
        }    
    }
}


pub async fn server(opts: Opts) {
    let state = Arc::new(Mutex::new(State::default()));
    {
        let mut guard = state.lock().unwrap();
        guard.tick_rate = Duration::from_millis(opts.tick_rate_ms);
        // guard.stride = opts.stride;
    }
    let (writer, mut reader, _) = io_channel::<Envelope<Payload>>();

    let state_cp = state.clone();
    let writer_cp = writer.clone();

    tokio::task::spawn(commit_buffered_delta_every_so_often(state_cp, writer_cp));

    while let Some(envelope) = reader.recv().await {
        handle_envelope(state.clone(), envelope, writer.clone()).await;
    }
}


#[tokio::main]
async fn main() {
    let opts = Opts::parse();

    tracing_subscriber::FmtSubscriber::builder()
    .with_writer(std::io::stderr)
    // .pretty()
    .with_ansi(false)
    .with_env_filter(EnvFilter::from_default_env())
    .init();

    debug!(opts = ?opts, "starting server...");
    server(opts).await;
}
