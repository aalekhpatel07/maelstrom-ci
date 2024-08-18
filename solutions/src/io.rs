use std::{fmt::Debug, io::{stdin, stdout, BufRead, Write}};
use tokio::{sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender}, task::JoinHandle};
use tracing::{error, trace};
use serde::{de::DeserializeOwned, Serialize};


pub fn io_channel<Message>() -> (UnboundedSender<Message>, UnboundedReceiver<Message>, JoinHandle<()>) 
where Message: Serialize + DeserializeOwned + Debug + Sync + Send + 'static
{

    let (input_tx, input_rx) = unbounded_channel();

    let read_handle = tokio::task::spawn(async move {
        let mut lines = std::io::BufReader::new(stdin()).lines();
        while let Some(Ok(line)) = lines.next() {
            trace!(num_bytes = line.as_bytes().len(), line = ?line, "read line");
            let Ok(message) = 
                serde_json::from_str(&line)
                .inspect_err(|err| {error!(error = ?err, "failed to deserialize line into message")}) 
            else {
                break;
            };
            trace!(message = ?message, "read message");
            if let Err(err) = input_tx.send(message) {
                error!(message = ?err, error = ?err, "No receiver is interested in listening to stdin. Dropping message");
                break;
            }
        }
    });

    let (output_tx, mut output_rx) = unbounded_channel::<Message>();

    let write_handle = tokio::task::spawn(async move {
        let mut stdout = std::io::BufWriter::new(stdout());
        while let Some(message) = output_rx.recv().await {
            trace!(message = ?message, "writing message");
            let Ok(line) = 
                serde_json::to_string(&message)
                .inspect_err(|err| {error!(error = ?err, "failed to serialize message")}) 
            else {
                break;
            };
            let bytes = line.as_bytes();
            trace!(num_bytes = bytes.len(), line = ?line, "writing line");
            if let Err(err) = stdout.write_all(bytes) {
                error!(message = ?err, error = ?err, "failed to write to stdout");
                break;
            }
            if let Err(err) = stdout.write_all(b"\n") {
                error!(message = ?err, error = ?err, "failed to write newline to stdout");
                break;
            }
            
            if let Err(err) = stdout.flush() {
                error!(error = ?err, "failed to flush to stdout");
            }
        }
    });

    let joined_handle = tokio::task::spawn(async move {
        let (read_result, write_result) = tokio::join!(read_handle, write_handle);
        read_result.unwrap();
        write_result.unwrap();
    });

    (output_tx, input_rx, joined_handle)
}