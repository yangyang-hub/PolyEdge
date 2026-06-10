#[tokio::main]
async fn main() -> polyedge_domain::Result<()> {
    polyedge_worker::run_cli().await
}
