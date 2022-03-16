use anyhow::Result;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::thread;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};

async fn ok(_: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::new(Body::empty()))
}

#[tokio::main]
pub async fn server_main(addr: SocketAddr) -> Result<()> {
    let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(ok)) });

    let server = Server::bind(&addr).serve(make_svc);

    server.await?;

    Ok(())
}

pub fn server_thread_start(addr: SocketAddr) -> Result<()> {
    thread::spawn(move || server_main(addr));
    Ok(())
}
