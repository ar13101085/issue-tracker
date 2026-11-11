use actix_web::{middleware, web, App, HttpServer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let cfg = issue_tracker::config::Config::from_env().unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    });
    let (host, port) = (cfg.host.clone(), cfg.port);
    let state = issue_tracker::state::init(cfg).await.unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    let state = web::Data::new(state);
    tracing::info!("listening on http://{host}:{port}");
    HttpServer::new(move || {
        App::new().wrap(middleware::Compress::default()).app_data(state.clone()).configure(issue_tracker::configure)
    })
    .bind((host.as_str(), port))?
    .run()
    .await
}
