use aether_core::runtime::{AetherNodeRuntime, CoreConfig};

use std::env;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Aether node failed: {:?}", error);

        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);

    let address = args.next().unwrap_or_else(|| "127.0.0.1:7000".to_owned());

    let address: SocketAddr = address.parse()?;

    let advertised_peers = args
        .filter_map(|value| value.parse().ok())
        .collect::<Vec<SocketAddr>>();

    let node = AetherNodeRuntime::bind(CoreConfig::default(), address).await?;

    println!("Aether node bound to {}", node.local_addr()?);

    if let Some(device) = node.core().device() {
        println!("Device ID: {:?}", device.device_id);
    }

    println!("Aether node running");

    node.run(advertised_peers).await?;

    println!("Aether node stopped");

    Ok(())
}
