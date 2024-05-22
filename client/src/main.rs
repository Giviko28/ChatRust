// A command line chat client will probably need some styling / high-level interaction with the command line. So this
// project includes `crossterm` by default, a Rust library for cross-plattform command line manipulation
/*
use crossterm::{
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    ExecutableCommand,
};

fn main() {
    // crossterm commands can fail. For now, you are allowed to use `.unwrap()`, but you can also take a look at error
    // handling in Rust and do it right from the start. We will talk about error handling probably towards the end of
    // May

    std::io::stdout()
        .execute(SetForegroundColor(Color::Blue))
        .unwrap()
        .execute(SetBackgroundColor(Color::Red))
        .unwrap()
        .execute(Print("Styled text here."))
        .unwrap()
        .execute(ResetColor)
        .unwrap();
}
*/

use futures::select;
use futures::FutureExt;

use async_std::{
    io::{stdin, BufReader},
    net::{TcpStream, ToSocketAddrs},
    prelude::*,
    task,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(crate) fn main() -> Result<()> {
    task::block_on(try_main("127.0.0.1:8888"))
}

async fn try_main(addr: impl ToSocketAddrs) -> Result<()> {
    let stream = TcpStream::connect(addr).await?;
    let (reader, mut writer) = (&stream, &stream);
    let reader = BufReader::new(reader);
    let mut lines_from_server = futures::StreamExt::fuse(reader.lines());

    let stdin = BufReader::new(stdin());
    let mut lines_from_stdin = futures::StreamExt::fuse(stdin.lines());
    loop {
        select! {
            line = lines_from_server.next().fuse() => match line {
                Some(line) => {
                    let line = line?;
                    println!("{}", line);
                },
                None => break,
            },
            line = lines_from_stdin.next().fuse() => match line {
                Some(line) => {
                    let line = line?;
                    writer.write_all(line.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                }
                None => break,
            }
        }
    }
    Ok(())
}
