use anyhow::Result;
use tokio::task::JoinHandle;

/// Run the server in on an existing async runtime
pub async fn serve(server_port: u16) -> Result<()> {
    warp::serve(super::routes::routes())
        .run(([0, 0, 0, 0], server_port))
        .await;

    Ok(())
}

/// For usage in sync context, spaws a dedicated tokio runtime
pub async fn thread_start(server_port: u16) -> JoinHandle<Result<()>> {
    tokio::spawn(async move { serve(server_port).await })
}
