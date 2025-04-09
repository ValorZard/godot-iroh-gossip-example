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
    ticket_string: GString,
    name: Option<GString>,
    remote_message_receiver: Option<tokio::sync::mpsc::Receiver<String>>,
    ticket_receiver: Option<tokio::sync::mpsc::Receiver<String>>,
    user_input_sender: Option<tokio::sync::mpsc::Sender<String>>,
}

#[godot_api]
impl INode for AsyncSingleton {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            ticket_string: "".into(),
            name: None,
            remote_message_receiver: None,
            ticket_receiver: None,
            user_input_sender: None,
        }
    }

    fn ready(&mut self) {}

    fn process(&mut self, delta: f64) {
        let mut self_gd = self.to_gd();
        if let Some(receiver) = &mut self.remote_message_receiver {
            while let Some(value) = receiver.try_recv().ok() {
                self_gd.signals()
                    .message_received()
                    .emit(GString::from(value));
            }
        }

        if let Some(receiver) = &mut self.ticket_receiver {
            while let Some(value) = receiver.try_recv().ok() {
                let ticket_string = GString::from(value);
                self.ticket_string = ticket_string.clone();
                self_gd
                    .signals()
                    .ticket_received()
                    .emit(ticket_string);
            }
        }
    }
}

#[godot_api]
impl AsyncSingleton {
    pub const SINGLETON: &'static str = "AsyncEventBus";

    #[signal]
    fn message_received(message: GString);

    #[signal]
    fn ticket_received(message: GString);

    #[func]
    pub fn hello(&self) {
        godot_print!("Hello from Async Singleton!"); // Prints to the Godot console
    }

    #[func]
    pub fn get_ticket(&mut self) -> GString {
        self.ticket_string.clone()
    }

    #[func]
    pub fn open_async_chat(&mut self) {
        let topic = TopicId::from_bytes(rand::random());

        self.start_gossip(topic, vec![]);
    }

    #[func]
    pub fn join_async_chat(&mut self, ticket: GString) {
        godot_print!("Joining async chat with ticket: {}", ticket);
        let Ticket { topic, nodes } = Ticket::from_str(&ticket.to_string()).unwrap();

        self.start_gossip(topic, nodes);
    }

    fn start_gossip(&mut self, topic: TopicId, nodes: Vec<NodeAddr>) {
        // create a multi-provider, single-consumer channel
        let (remote_message_tx, remote_message_rx) = tokio::sync::mpsc::channel::<String>(1);
        self.remote_message_receiver = Some(remote_message_rx);
        let (input_tx, mut input_rx) = tokio::sync::mpsc::channel::<String>(1);
        self.user_input_sender = Some(input_tx);

        let name = match &self.name {
            Some(name) => Some(name.to_string()),
            None => None,
        };

        let (ticket_tx, ticket_rx) = tokio::sync::mpsc::channel::<String>(1);
        self.ticket_receiver = Some(ticket_rx);

        AsyncRuntime::spawn(async move {
            let endpoint = Endpoint::builder().discovery_n0().bind().await.unwrap();

            println!("> our node id: {}", endpoint.node_id());
            let gossip = Gossip::builder().spawn(endpoint.clone()).await.unwrap();

            let router = Router::builder(endpoint.clone())
                .accept(iroh_gossip::ALPN, gossip.clone())
                .spawn()
                .await
                .unwrap();

            // in our main file, after we create a topic `id`:
            // print a ticket that includes our own node id and endpoint addresses
            let ticket = {
                // Get our address information, includes our
                // `NodeId`, our `RelayUrl`, and any direct
                // addresses.
                let me = endpoint.node_addr().await.unwrap();
                let nodes = vec![me];
                Ticket { topic, nodes }
            };
            println!("> ticket to join us: {ticket}");
            ticket_tx.send(ticket.to_string()).await.unwrap();

            // join the gossip topic by connecting to known nodes, if any
            let node_ids = nodes.iter().map(|p| p.node_id).collect();
            if nodes.is_empty() {
                println!("> waiting for nodes to join us...");
            } else {
                println!("> trying to connect to {} nodes...", nodes.len());
                // add the peer addrs from the ticket to our endpoint's addressbook so that they can be dialed
                for node in nodes.into_iter() {
                    endpoint.add_node_addr(node).unwrap();
                }
            };
            let (sender, receiver) = gossip
                .subscribe_and_join(topic, node_ids)
                .await
                .unwrap()
                .split();
            println!("> connected!");

            // broadcast our name, if set
            if let Some(name) = name {
                let message = Message::AboutMe {
                    from: endpoint.node_id(),
                    name,
                };
                sender.broadcast(message.to_vec().into()).await.unwrap();
            }

            // subscribe and print loop
            tokio::spawn(subscribe_loop(receiver, remote_message_tx));

            // broadcast each line we type
            println!("> type a message and hit enter to broadcast...");
            // listen for lines that we have typed to be sent from `stdin`
            while let Some(text) = input_rx.recv().await {
                // create a message from the text
                let message = Message::Message {
                    from: endpoint.node_id(),
                    text: text.clone(),
                };
                // broadcast the encoded message
                sender.broadcast(message.to_vec().into()).await.unwrap();
                // print to ourselves the text that we sent
                println!("> sent: {text}");
            }
            router.shutdown().await.unwrap();
        });
    }

    #[func]
    pub fn poll_receiver(&mut self) -> Array<GString> {
        let mut array = Array::new();
        if let Some(receiver) = &mut self.remote_message_receiver {
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
        let sender = self.user_input_sender.clone();
        if sender.is_none() {
            godot_print!("Sender is not initialized!");
            return;
        }
        AsyncRuntime::spawn(async {
            sender.unwrap().send(string).await.unwrap();
        });
    }
}
