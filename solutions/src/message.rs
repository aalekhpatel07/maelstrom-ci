use serde::{Serialize, Deserialize};


#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope<M> {
    #[serde(rename = "src")]
    pub source: String,
    #[serde(rename = "dest")]
    pub destination: String,
    pub body: Body<M>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body<M> {

    /// The id that the client gives us for any rpc it makes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<usize>,
    /// The message our rpc response corresponds to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<usize>,

    /// The actual payload.
    #[serde(flatten)]
    pub message: M,
}


impl<M> Envelope<M> {
    pub fn new(src: &str, dest: &str, body: Body<M>) -> Self {
        Self {
            source: src.to_owned(),
            destination: dest.to_owned(),
            body
        }
    }
    pub fn reply_with(&self, msg_id: Option<usize>, message: M) -> Self {
        Self {
            source: self.destination.clone(),
            destination: self.source.clone(),
            body: Body {
                msg_id,
                in_reply_to: self.body.msg_id,
                message
            }
        }
    }
    pub fn msg_id(&self) -> Option<usize> {
        self.body.msg_id
    }
}