mod cli;
mod ping;
mod shutdown;
mod tcp;
mod udp;

fn main() -> std::io::Result<()> {
    println!("quote-client: stub");
    // TODO: прочитать тикеры -> отправить STREAM -> UDP receive + ping thread
    Ok(())
}
