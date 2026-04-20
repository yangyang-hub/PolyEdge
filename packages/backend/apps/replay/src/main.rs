use polyedge_infrastructure::{Runtime, telemetry::init_tracing};
use tracing::info;

#[tokio::main]
async fn main() -> polyedge_domain::Result<()> {
    init_tracing("polyedge_replay");
    let runtime = Runtime::load().await?;
    let state = runtime.app_state();

    info!(
        environment = %state.settings.runtime.environment,
        current_mode = ?state.settings.runtime.initial_mode,
        "polyedge replay runtime initialized",
    );
    info!("replay binary skeleton is ready for fixture-driven research jobs");
    Ok(())
}
