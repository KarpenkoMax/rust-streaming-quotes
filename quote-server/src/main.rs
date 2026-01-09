mod config;
mod generator;
mod hub;
mod session;
mod tcp;

fn main() -> std::io::Result<()> {
    let tickers = config::load_server_tickers(None)?;
    println!("quote-server: loaded {} tickers", tickers.len());
    // TODO: старт TCP listener, генератора и клиентских сессий
    Ok(())
}
