use fortochka_server::{AppState, ServerConfig, app};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".parse().expect("валидный фильтр")),
        )
        .init();

    let config = ServerConfig::from_env();
    let state = AppState::init(&config.data_dir).await?;
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, data_dir = %config.data_dir.display(), "форточка-сервер слушает");

    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Docker шлёт SIGTERM при остановке контейнера — завершаемся мягко,
/// не обрывая запросы на середине.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("обработчик Ctrl+C");
    };
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("обработчик SIGTERM")
            .recv()
            .await;
    };
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("получен сигнал остановки");
}
