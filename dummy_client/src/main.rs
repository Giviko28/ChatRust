use async_std::{
    io::{stdin, BufReader},
    net::{TcpStream, ToSocketAddrs},
    prelude::*,
    task,
};
use crossterm::{
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    ExecutableCommand,
};
use futures::select;
use futures::FutureExt;
use std::time::{Duration, Instant};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(crate) fn main() -> Result<()> {
    task::block_on(try_main("127.0.0.1:8888", "User1"))
}

async fn try_main(addr: impl ToSocketAddrs, username: &str) -> Result<()> {
    let stream = TcpStream::connect(addr).await?;
    let (reader, mut writer) = (&stream, &stream);
    let reader = BufReader::new(reader);
    let mut lines_from_server = futures::StreamExt::fuse(reader.lines());

    let stdin = BufReader::new(stdin());
    let mut lines_from_stdin = futures::StreamExt::fuse(stdin.lines());

    // Send username to the server
    writer.write_all(username.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    let mut start_time = Instant::now();

    loop {
        select! {
            line = lines_from_server.next().fuse() => match line {
                Some(line) => {
                    let line = line?;
                    std::io::stdout()
                        .execute(SetForegroundColor(Color::Black))
                        .unwrap()
                        .execute(SetBackgroundColor(Color::White))
                        .unwrap()
                        .execute(ResetColor)
                        .unwrap()
                        .execute(Print(format!("{}\n", line)))
                        .unwrap();

                    // Measure the time to receive the message back
                    let elapsed = start_time.elapsed();
                    println!("Time ends!");
                    println!("Time to receive message: {}.{} seconds",
                             elapsed.as_secs(),
                             elapsed.subsec_millis());
                },
                None => break,
            },
            line = lines_from_stdin.next().fuse() => match line {
                Some(line) => {
                    let line = line?;
                    start_time = Instant::now();
                    println!("Time starts!");
                    writer.write_all(line.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                }
                None => break,
            }
        }
    }

    Ok(())
}
