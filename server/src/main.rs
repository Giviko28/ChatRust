// Some thoughts on the chat server:
// - This is a server, so an executable that runs perpetually! So there will be a loop, maybe? What will that loop do?
// - At some point, you want to configure your server: Where should it run? Maybe limit the number of concurrent users? What else would you like to configure? How would you do the configuring?
// - Users should be able to message each other. What types of messaging do you want to support? Only one-on-one or also rooms/groups etc? How will the messages look like? Should users be able to send each other files?
// - What job does the server have when it comes to messages? Does it only facilitate peer-to-peer communication between clients, or do all messages go through the server?
//   - What would be the benefits and drawbacks of each approach?
// - Do you want/need some form of user management? If so, how would that look like?

extern crate async_std;
#[macro_use]
extern crate lazy_static;
use async_std::{
    io::BufReader,
    net::TcpStream,
    net::{TcpListener, ToSocketAddrs}, // a non-blocking tcp-listener (uses async API)
    prelude::*,                        // needed to work with futures and streams
    task, // a lightweight alternative to a thread (a thread can have many tasks)
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;
use futures::channel::mpsc;
use std::sync::Mutex;
use futures::sink::SinkExt;
use futures::{select, FutureExt};
use postgres::{Client, NoTls};
use std::{
    collections::hash_map::{Entry, HashMap},
    future::Future,
    sync::Arc,
};
use std::fmt::Error;
use futures::channel::mpsc::UnboundedReceiver;

fn spawn_and_log_error<F>(fut: F) -> task::JoinHandle<()>
// logging errors but continuing maintaining the server
where
    F: Future<Output = Result<()>> + Send + 'static,
{
    task::spawn(async move {
        if let Err(e) = fut.await {
            eprintln!("{}", e)
        }
    })
}

async fn accept_loop(addr: impl ToSocketAddrs) -> Result<()> {
    // has to be async to be able to use await syntax
    let listener = TcpListener::bind(addr).await?; // bind returns future -> has to be awaited

    let (broker_sender, broker_receiver) = mpsc::unbounded();
    let broker_handle = task::spawn(broker_loop(broker_receiver));
    let mut incoming = listener.incoming();

    while let Some(stream) = incoming.next().await {
        // iterate incoming sockets
        let stream = stream?;
        println!("Accepting connection from: \"{}\"", stream.peer_addr()?);
        spawn_and_log_error(connection_loop(broker_sender.clone(), stream)); // task::spawn creates a task (to work with each client concurrently)
    }
    drop(broker_sender);
    broker_handle.await?;
    Ok(())
}

async fn connection_loop(mut broker: Sender<Event>, stream: TcpStream) -> Result<()> {
    let mut test = stream.clone();
    let stream = Arc::new(stream);
    let reader = BufReader::new(&*stream); // incoming stream is read
    let mut lines = reader.lines(); // split incoming streams into lines (each line is a stringstream)

    let name = match lines.next().await {
        // first line is read
        None => Err("peer disconnected immediately")?,
        Some(line) => line?,
    };


    if (is_new_user(&name)) {
        println!("new user created: \"{}\"", name);
        let t_name = name.clone();
        // basically, I'm not using async postgresql, so to avoid blocking the app I spawn a separate thread
        std::thread::spawn(move || {
            save_user(&*t_name).expect("TODO: panic message");
        });
        USERS_IN_DB.lock().unwrap().push(name.clone());
    } else {
        println!("An old user is back! his username is: \"{}\"", name);
    }

    let (shutdown_sender, shutdown_receiver) = mpsc::unbounded::<Void>();
    broker
        .send(Event::NewPeer {
            name: name.clone(),
            stream: Arc::clone(&stream),
            shutdown: shutdown_receiver,
        })
        .await
        .unwrap();

    while let Some(line) = lines.next().await {
        let line = line?;
        let (dest, msg) = match line.find(':') {
            // parsing each line into into destination list and message (The message format is -> Bob: Hello Bob)
            None => continue,
            Some(idx) => (&line[..idx], line[idx + 1..].trim()),
        };
        let dest: Vec<String> = dest
            .split(',')
            .map(|name| name.trim().to_string())
            .collect(); // a vector of strings because many destinations can be given seperated by comma
        let msg: String = msg.to_string();

        broker
            .send(Event::Message {
                from: name.clone(),
                to: dest,
                msg,
            })
            .await
            .unwrap();
    }
    Ok(())
}

async fn connection_writer_loop(
    // for serializing the messages, so they don't interfere with each other
    messages: &mut Receiver<String>,
    stream: Arc<TcpStream>,
    shutdown: Receiver<Void>,
) -> Result<()> {
    let mut stream = &*stream;
    let mut messages = messages.fuse();
    let mut shutdown = shutdown.fuse();
    loop {
        select! {
            msg = messages.next().fuse() => match msg {
                Some(msg) => stream.write_all(msg.as_bytes()).await?,
                None => break,
            },
            void = shutdown.next().fuse() => match void {
                Some(void) => match void {},
                None => break,
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
enum Void {}

#[derive(Debug)]
enum Event {
    // 2 kinds of events: a new user or a message
    NewPeer {
        name: String,
        stream: Arc<TcpStream>,
        shutdown: Receiver<Void>,
    },
    Message {
        from: String,
        to: Vec<String>,
        msg: String,
    },
}

async fn broker_loop(events: Receiver<Event>) -> Result<()> {
    // make sure, messages read in "connection_loop" get to relevant "connection_writer_loop"
    let (disconnect_sender, mut disconnect_receiver) =
        mpsc::unbounded::<(String, Receiver<String>)>();
    let mut peers: HashMap<String, Sender<String>> = HashMap::new(); // maintaining all peers (users)
    let mut events = events.fuse();
    loop {
        let event = select! {
            event = events.next().fuse() => match event {
                None => break, // 2
                Some(event) => event,
            },
            disconnect = disconnect_receiver.next().fuse() => {
                let (name, _pending_messages) = disconnect.unwrap();
                assert!(peers.remove(&name).is_some());
                continue;
            },
        };
        match event {
            Event::Message { from, to, msg } => {
                for addr in to {
                    if let Some(peer) = peers.get_mut(&addr) {
                        println!("a Message was sent by \"{}\" to \"{}\"", from, addr);
                        let formatted_msg = format!("Message from \"{}\": {}\n", from, msg);
                        peer.send(formatted_msg).await.unwrap();
                        // i tried passing by reference and it started complaining about lifetimes, aint no way im fixing that
                        save_message(from.clone(), addr.clone(), msg.clone()).unwrap()
                    }
                }
            }
            Event::NewPeer {
                name,
                stream,
                shutdown,
            } => match peers.entry(name.clone()) {
                Entry::Occupied(..) => (),
                Entry::Vacant(entry) => {
                    let (client_sender, mut client_receiver) = mpsc::unbounded();
                    entry.insert(client_sender);
                    let mut disconnect_sender = disconnect_sender.clone();
                    spawn_and_log_error(async move {
                        let res =
                            connection_writer_loop(&mut client_receiver, stream, shutdown).await;
                        disconnect_sender
                            .send((name, client_receiver))
                            .await
                            .unwrap();
                        res
                    });
                }
            },
        }
    }
    drop(peers);
    drop(disconnect_sender);
    while let Some((_name, _pending_messages)) = disconnect_receiver.next().await {}
    Ok(())
}

lazy_static! {
    static ref DB_CONNECTION: Mutex<Client> = Mutex::new(
        Client::connect("host=localhost port=7777 user=postgres password=mysecretpassword dbname=postgres", NoTls).unwrap()
    );
    static ref USERS_NOW: HashMap<String, Sender<String>> = HashMap::new();
    static ref USERS_IN_DB: Mutex<Vec<String>> = Mutex::new(load_users());
}


fn load_users() -> Vec<String> {
    let mut db = DB_CONNECTION.lock().unwrap();
    let mut names = Vec::new();

    for row in db.query("SELECT name FROM users", &[]).unwrap() {
        let name: String = row.get(0);
        names.push(name);
    }

    names
}

fn is_new_user(name: &str) -> bool {
    !USERS_IN_DB.lock().unwrap().contains(&name.to_string())
}

fn save_user(name: &str) -> Result<()> {
    let mut db = DB_CONNECTION.lock().unwrap();

    // Prepare the SQL statement to insert a new user into the 'users' table
    let statement = db.prepare("INSERT INTO users (name) VALUES ($1)")?;

    // Execute the SQL statement with the user's name as a parameter
    db.execute(&statement, &[&name])?;

    Ok(())
}

fn save_message(from: String, to: String, msg: String) -> Result<()> {
    std::thread::spawn(move || {
        let mut db = DB_CONNECTION.lock().unwrap();
        db.execute("INSERT INTO messages (sender, receiver, message) VALUES ($1, $2, $3)", &[&from, &to, &msg]).unwrap();
    }).join().unwrap();;
    Ok(())
}

pub(crate) fn main() -> Result<()> {
    task::block_on(accept_loop("127.0.0.1:8888"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /*Server tests*/
    #[test]
    fn test_spawn_and_log_error() {}

    #[test]
    fn test_accept_loop() {}

    #[test]
    fn test_connection_loop() {}

    #[test]
    fn test_connection_writer_loop() {}

    #[test]
    fn test_broker_loop() {}

    #[test]
    fn test_load_users() {}

    #[test]
    fn test_is_new_user() {}

    #[test]
    fn test_save_message() {}
}