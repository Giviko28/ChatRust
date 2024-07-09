// Some thoughts on the chat server:
// - This is a server, so an executable that runs perpetually! So there will be a loop, maybe? What will that loop do?
// - At some point, you want to configure your server: Where should it run? Maybe limit the number of concurrent users? What else would you like to configure? How would you do the configuring?
// - Users should be able to message each other. What types of messaging do you want to support? Only one-on-one or also rooms/groups etc? How will the messages look like? Should users be able to send each other files?
// - What job does the server have when it comes to messages? Does it only facilitate peer-to-peer communication between clients, or do all messages go through the server?
//   - What would be the benefits and drawbacks of each approach?
// - Do you want/need some form of user management? If so, how would that look like?
mod server_tui;
static mut CHAT_UI: Option<server_tui::ChatUI<CrosstermBackend<io::Stdout>>> = None;

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
use futures::sink::SinkExt;
use futures::{select, FutureExt};
use postgres::{Client, NoTls};
use std::{
    collections::hash_map::{Entry, HashMap},
    future::Future,
    io,
    sync::Arc,
};
use std::{fmt::format, sync::Mutex};
use tui::{backend::CrosstermBackend, Terminal};

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
        let message = format!("Accepting connection from: \"{}\"", stream.peer_addr()?);
        unsafe { CHAT_UI.as_mut().unwrap().push_message(message.into()) }
        //println!("Accepting connection from: \"{}\"", stream.peer_addr()?);
        spawn_and_log_error(connection_loop(broker_sender.clone(), stream)); // task::spawn creates a task (to work with each client concurrently)
    }
    drop(broker_sender);
    broker_handle.await?;
    Ok(())
}

async fn connection_loop(mut broker: Sender<MessageEvent>, stream: TcpStream) -> Result<()> {
    let _test = stream.clone();
    let stream = Arc::new(stream);
    let reader = BufReader::new(&*stream); // incoming stream is read
    let mut lines = reader.lines(); // split incoming streams into lines (each line is a stringstream)

    let name = match lines.next().await {
        None => {
            // Handle the disconnection gracefully
            let message = "Peer disconnected immediately";
            unsafe { CHAT_UI.as_mut().unwrap().push_error(message.into()) }
            return Err("Peer disconnected immediately".into());
        }
        Some(line) => line?,
    };

    if is_new_user(&name) {
        println!("new user created: \"{}\"", name);
        let t_name = name.clone();
        // basically, I'm not using async postgresql, so to avoid blocking the app I spawn a separate thread
        let _join_handle = std::thread::spawn(move || save_user(&t_name));
        //USERS_IN_DB.lock().unwrap().push(name.clone());
        match USERS_IN_DB.lock() {
            Ok(mut users) => {
                users.push(name.clone());
            }
            Err(e) => {
                unsafe {
                    CHAT_UI.as_mut().unwrap().push_error(
                        format!("Failed to acquire lock on the DB for user {}: {}", name, e).into(),
                    )
                }
                //eprintln!("Failed to acquire lock on the DB for user {}: {}", name, e);
            }
        }
    } else {
        unsafe {
            CHAT_UI
                .as_mut()
                .unwrap()
                .push_message(format!("An old user is back! his username is: {}", name).into())
        }
        //println!("An old user is back! his username is: \"{}\"", name);
    }

    let (_shutdown_sender, shutdown_receiver) = mpsc::unbounded::<Void>();
    broker
        .send(MessageEvent::NewPeer {
            name: name.clone(),
            stream: Arc::clone(&stream),
            shutdown: shutdown_receiver,
        })
        .await
        .map_err(|e| {
            eprintln!("Failed to send NewPeer event: {}", e);
        })
        .ok();

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
            .send(MessageEvent::Message {
                from: name.clone(),
                to: dest,
                msg,
            })
            .await
            .map_err(|e| {
                eprintln!("Failed to send Message: {}", e);
            })
            .ok();
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
enum MessageEvent {
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

async fn broker_loop(events: Receiver<MessageEvent>) -> Result<()> {
    // make sure, messages read in "connection_loop" get to relevant "connection_writer_loop"
    let (disconnect_sender, mut disconnect_receiver) =
        mpsc::unbounded::<(String, Receiver<String>)>();
    let mut peers: HashMap<String, Sender<String>> = HashMap::new(); // maintaining all peers (users)
    let mut events = events.fuse();
    loop {
        let event = select! {
            event = events.next().fuse() => match event {
                None => break,
                Some(event) => event,
            },
            disconnect = disconnect_receiver.next().fuse() => {
                //let (name, _pending_messages) = disconnect.unwrap();
                let (name, _) = disconnect.expect("Failed to disconnect");
                if peers.remove(&name).is_none() {
                    eprintln!("User with name '{}' not found in the userlist", name);
                }
                continue;
            },
        };
        match event {
            MessageEvent::Message { from, to, msg } => {
                let source = from.clone();
                let source2 = from.clone();
                let message = msg.clone();
                for addr in to {
                    if let Some(peer) = peers.get_mut(&addr) {
                        unsafe {
                            CHAT_UI.as_mut().unwrap().push_message(
                                format!("a Message was sent by \"{}\" to \"{}\"", source, addr)
                                    .into(),
                            )
                        }
                        //println!("a Message was sent by \"{}\" to \"{}\"", source, addr);
                        let formatted_msg =
                            format!("Message from \"{}\": {}\n", source, msg.clone());
                        //peer.send(formatted_msg).await.unwrap();
                        if let Err(e) = peer.send(formatted_msg).await {
                            eprintln!("Failed to send message to peer: {}", e);
                        }
                        // i tried passing by reference and it started complaining about lifetimes, aint no way im fixing that
                        save_message(&source2, &addr, &message).unwrap_or_else(|e| {
                            eprintln!("Failed to save message: {}", e);
                        });
                    }
                }
            }
            MessageEvent::NewPeer {
                name,
                stream,
                shutdown,
            } => match peers.entry(name.clone()) {
                Entry::Occupied(..) => unsafe {
                    CHAT_UI
                        .as_mut()
                        .unwrap()
                        .push_error("User already exists!".into())
                },
                Entry::Vacant(entry) => {
                    let (client_sender, mut client_receiver) = mpsc::unbounded();
                    entry.insert(client_sender);
                    let mut disconnect_sender = disconnect_sender.clone();
                    let name_clone = name.clone();
                    spawn_and_log_error(async move {
                        let res =
                            connection_writer_loop(&mut client_receiver, stream, shutdown).await;
                        disconnect_sender
                            .send((name_clone, client_receiver))
                            .await
                            .expect("Failed to send disconnect event");
                        res
                    });
                    unsafe {
                        CHAT_UI.as_mut().unwrap().add_user(name.into());
                    }
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
        Client::connect(
            "host=localhost port=7777 user=postgres password=mysecretpassword dbname=postgres",
            NoTls
        )
        .unwrap()
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

fn save_message(from: &str, to: &str, msg: &str) -> Result<()> {
    let from_str = from.to_string();
    let to_str = to.to_string();
    let msg_str = msg.to_string();

    let join_handle = std::thread::spawn(move || {
        let mut db = DB_CONNECTION.lock().unwrap();
        db.execute(
            "INSERT INTO messages (sender, receiver, message) VALUES ($1, $2, $3)",
            &[&from_str, &to_str, &msg_str],
        )
        .unwrap();
    });

    join_handle
        .join()
        .expect("Failed to join the save_message thread");

    Ok(())
}

pub(crate) fn main() -> Result<()> {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.hide_cursor()?;

    //terminal.clear()?; // Clear the terminal before getting the frame
    let chat_ui = server_tui::ChatUI::new(terminal);

    unsafe {
        CHAT_UI = Some(chat_ui);

        CHAT_UI.as_mut().unwrap().draw();
    }

    task::block_on(accept_loop("127.0.0.1:8888"))
}
