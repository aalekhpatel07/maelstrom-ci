use serde::{Serialize, Deserialize};


#[derive(Debug, Serialize, Deserialize)]
pub struct Message<Body> {
    #[serde(rename = "src")]
    pub source: String,
    #[serde(rename = "dest")]
    pub destination: String,
    pub body: Body
}


impl<Body> Message<Body> {
    pub fn new(src: &str, dest: &str, body: Body) -> Self {
        Self {
            source: src.to_owned(),
            destination: dest.to_owned(),
            body
        }
    }
    pub fn reply_with(&self, body: Body) -> Self {
        Self {
            source: self.destination.clone(),
            destination: self.source.clone(),
            body
        }
    }
}