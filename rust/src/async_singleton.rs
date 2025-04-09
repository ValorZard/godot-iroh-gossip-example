use std::{collections::HashMap, fmt, str::FromStr};

use anyhow::Ok;
use futures_lite::StreamExt;
use godot::{classes::Engine, prelude::*};
use iroh::{Endpoint, NodeAddr, NodeId, protocol::Router};
use iroh_gossip::{
    net::{Event, Gossip, GossipEvent, GossipReceiver, GossipSender},
    proto::TopicId,
};
use serde::{Deserialize, Serialize};

use crate::async_runtime::AsyncRuntime;

type SendData = i32;

// add the `Ticket` code to the bottom of the main file
#[derive(Debug, Serialize, Deserialize)]
struct Ticket {
    topic: TopicId,
    nodes: Vec<NodeAddr>,
}

impl Ticket {
    /// Deserialize from a slice of bytes to a Ticket.
    fn from_bytes(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        serde_json::from_slice(bytes).map_err(Into::into)
    }

    /// Serialize from a `Ticket` to a `Vec` of bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("serde_json::to_vec is infallible")
    }
}

// The `Display` trait allows us to use the `to_string`
// method on `Ticket`.
impl fmt::Display for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut text = data_encoding::BASE32_NOPAD.encode(&self.to_bytes()[..]);
        text.make_ascii_lowercase();
        write!(f, "{}", text)
    }
}

// The `FromStr` trait allows us to turn a `str` into
// a `Ticket`
impl FromStr for Ticket {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = data_encoding::BASE32_NOPAD.decode(s.to_ascii_uppercase().as_bytes())?;
        Self::from_bytes(&bytes)
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum Message {
    AboutMe { from: NodeId, name: String },
    Message { from: NodeId, text: String },
}

impl Message {
    fn from_bytes(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        serde_json::from_slice(bytes).map_err(Into::into)
    }

    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("serde_json::to_vec is infallible")
    }
}

// Handle incoming events
async fn subscribe_loop(
    mut receiver: GossipReceiver,
    message_tx: tokio::sync::mpsc::Sender<String>,
) -> Result<(), anyhow::Error> {
    // keep track of the mapping between `NodeId`s and names
    let mut names = HashMap::new();
    // iterate over all events
    while let Some(event) = receiver.try_next().await? {
        // if the Event is a `GossipEvent::Received`, let's deserialize the message:
        if let Event::Gossip(GossipEvent::Received(msg)) = event {
            // deserialize the message and match on the
            // message type:
            match Message::from_bytes(&msg.content)? {
                Message::AboutMe { from, name } => {
                    // if it's an `AboutMe` message
                    // add and entry into the map
                    // and print the name
                    names.insert(from, name.clone());
                    message_tx
                        .send(format!("> {} is now known as {}", from.fmt_short(), name))
                        .await?;
                }
                Message::Message { from, text } => {
                    // if it's a `Message` message,
                    // get the name from the map
                    // and print the message
                    let name = names
                        .get(&from)
                        .map_or_else(|| from.fmt_short(), String::to_string);
                    message_tx.send(format!("{}: {}", name, text)).await?;
                }
            }
        }
    }
    Ok(())
}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct AsyncSingleton {
    base: Base<Node>,
    receiver: Option<tokio::sync::mpsc::Receiver<SendData>>,
    endpoint: Option<Endpoint>,
    router: Option<Router>,
    ticket: Option<Ticket>,
    sender: Option<GossipSender>,
    name: Option<GString>,
    nodes: Vec<NodeAddr>,
    topic: Option<TopicId>,
    remote_message_rx: Option<tokio::sync::mpsc::Receiver<String>>,
    user_input_sender: Option<GossipSender>,
}

#[godot_api]
impl INode for AsyncSingleton {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            receiver: None,
            endpoint: None,
            router: None,
            ticket: None,
            sender: None,
            name: None,
            nodes: vec![],
            topic: None,
            remote_message_rx: None,
            user_input_sender: None,
        }
    }

    /*
    fn process(&mut self, delta: f64) {
        if let Some(receiver) = &mut self.receiver {
            while let Some(value) = receiver.try_recv().ok() {
                godot_print!("Received value: {}", value);
            }
        }
    }
    */
}

#[godot_api]
impl AsyncSingleton {
    pub const SINGLETON: &'static str = "AsyncEventBus";

    /// This function has no real use for the user, only to make it easier
    /// for this crate to access the singleton object.
    /*
    pub fn singleton() -> Option<Gd<AsyncEventBus>> {
        match Engine::singleton().get_singleton(Self::SINGLETON) {
            Some(singleton) => Some(singleton.cast::<Self>()),
            None => None,
        }
    }
    */

    #[func]
    pub fn get_ticket(&mut self) -> GString{
        if let Some(ticket) = &self.ticket {
            let ticket_str = GString::from(ticket.to_string());
            return ticket_str;
        }
        GString::from("")
    }

    #[func]
    pub fn open_async_chat(&mut self) {
        self.topic = Some(TopicId::from_bytes(rand::random()));

        self.start_gossip();
    }

    #[func]
    pub fn join_async_chat(&mut self, ticket: GString) {
        let Ticket { topic, nodes } = Ticket::from_str(&ticket.to_string()).unwrap();
        self.topic = Some(topic);
        self.nodes = nodes;

        self.start_gossip();
    }

    fn start_gossip(&mut self) {
        // create a multi-provider, single-consumer channel
        let (remote_message_tx, remote_message_rx) = tokio::sync::mpsc::channel::<String>(1);
        self.remote_message_rx = Some(remote_message_rx);
        AsyncRuntime::block_on(async {
            let endpoint = Endpoint::builder().discovery_n0().bind().await.unwrap();

            println!("> our node id: {}", endpoint.node_id());
            let gossip = Gossip::builder().spawn(endpoint.clone()).await.unwrap();

            let router = Router::builder(endpoint.clone())
                .accept(iroh_gossip::ALPN, gossip.clone())
                .spawn()
                .await
                .unwrap();

            self.router = Some(router);

            // in our main file, after we create a topic `id`:
            // print a ticket that includes our own node id and endpoint addresses
            let ticket = {
                // Get our address information, includes our
                // `NodeId`, our `RelayUrl`, and any direct
                // addresses.
                let me = endpoint.node_addr().await.unwrap();
                let nodes = vec![me];
                Ticket {
                    topic: self.topic.unwrap(),
                    nodes,
                }
            };
            println!("> ticket to join us: {ticket}");
            self.ticket = Some(ticket);
        });
    }

    #[func]
    pub fn poll_receiver(&mut self) -> Array<GString> {
        let mut array = Array::new();
        if let Some(receiver) = &mut self.remote_message_rx {
            while let Some(value) = receiver.try_recv().ok() {
                //godot_print!("Received value: {}", value);
                let message = GString::from(value);
                array.push(&message);
            }
        } else {
            godot_print!("Receiver is not initialized!");
        }
        array
    }

    #[func]
    pub fn send_message(&self, message: GString) {
        let string = message.to_string();
        let bytes = string.as_bytes();
        AsyncRuntime::block_on(async {
            self.sender
                .as_ref()
                .unwrap()
                .broadcast(bytes.to_vec().into())
                .await
                .unwrap();
        });
    }
}
