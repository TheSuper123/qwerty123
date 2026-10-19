use anyhow::{Context, Result};
use clap::Parser;
use rand::RngCore;
use std::{
    net::{Ipv4Addr, SocketAddrV4},
    path::PathBuf,
};

#[derive(Parser)]
#[command(
    name = "MostServer",
    version,
    about = "MostServer — osobisty instalator aplikacji iPhone na własnym PC"
)]
struct Options {
    #[arg(long, default_value_t = 49240)]
    port: u16,
    #[arg(long)]
    data_dir: Option<PathBuf>,
    #[arg(long)]
    no_browser: bool,
    #[arg(long)]
    self_test: bool,
    #[arg(long)]
    licenses: bool,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("MostServer: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let options = Options::parse();
    if options.licenses {
        println!(
            "{}",
            include_str!(concat!(env!("OUT_DIR"), "/licenses.txt"))
        );
        return Ok(());
    }
    std::panic::set_hook(Box::new(|_| {
        eprintln!(
            "MostServer: silnik przerwał operację. Sprawdź wynik zadania; poufne szczegóły nie są wypisywane."
        )
    }));
    let _ = rustls::crypto::ring::default_provider().install_default();
    isideload::init()
        .map_err(|_| anyhow::anyhow!("Nie można zainicjować biblioteki podpisywania."))?;
    if options.self_test {
        let root = tempfile::tempdir()?;
        let store = most_server::store::Store::open(root.path())?;
        anyhow::ensure!(
            store.artifacts()?.is_empty() && store.jobs()?.is_empty(),
            "Błąd magazynu danych."
        );
        anyhow::ensure!(
            most_server::account::validate_anisette("http://127.0.0.1:6969", false).is_ok(),
            "Błąd walidacji ustawień."
        );
        println!(
            "MostServer {}: storage and configuration self-test passed (no device test).",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    let root = options
        .data_dir
        .or_else(|| dirs::data_local_dir().map(|d| d.join("MostServer")))
        .context("Nie znaleziono katalogu danych użytkownika. Użyj --data-dir.")?;
    let listener=tokio::net::TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST,options.port)).await.context("Nie można otworzyć lokalnego portu. Zamknij poprzedni MostServer albo użyj --port z innym numerem.")?;
    let authority = listener.local_addr()?.to_string();
    let mut token = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut token);
    let state = most_server::state::AppState::new(&root, authority.clone(), hex::encode(token))
        .context("Nie można otworzyć bazy danych lub bezpiecznego magazynu kluczy.")?;
    let url = format!("http://{authority}/#token={}", state.token);
    println!("MostServer {} · Windows preview", env!("CARGO_PKG_VERSION"));
    println!(
        "Panel jest dostępny tylko na tym komputerze. Link zawiera lokalny klucz dostępu; nie udostępniaj go."
    );
    println!("{url}");
    println!("Zamknij program przez Ctrl+C lub przycisk Zakończ w panelu.");
    if !options.no_browser {
        let _ = open::that_detached(&url);
    }
    let shutdown_state = state.clone();
    axum::serve(listener,most_server::api::router(state)).with_graceful_shutdown(async move {
        tokio::select! { _=tokio::signal::ctrl_c()=>(),_=shutdown_state.shutdown.notified()=>() }
    }).await?;
    Ok(())
}
