use tracing_subscriber::{EnvFilter, fmt};

pub fn init_tracing(service_name: &str) {
    let env_filter = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => EnvFilter::new(format!("{service_name}=debug,tower_http=info,sqlx=info")),
    };

    let _ = fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .compact()
        .try_init();
}
