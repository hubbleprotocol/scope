use std::thread;

use anyhow::Result;

/// Run the server in on an existing async runtime
pub async fn serve(server_port: u16) -> Result<()> {
    warp::serve(super::routes::routes())
        .run(([0, 0, 0, 0], server_port))
        .await;

    Ok(())
}

/// For usage in sync context, spaws a dedicated tokio runtime
pub fn thread_start(server_port: u16) -> Result<()> {
    thread::spawn(move || server_main(server_port));
    Ok(())
}

/// Internal async main for sync apps
#[tokio::main]
async fn server_main(server_port: u16) -> Result<()> {
    serve(server_port).await?;

    Ok(())
}
