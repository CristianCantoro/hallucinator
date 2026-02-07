use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = axum::Router::new()
        .route("/", axum::routing::get(index));

    let addr = SocketAddr::from(([0, 0, 0, 0], 5001));
    println!("Listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index() -> &'static str {
    "hallucinator-web: not yet implemented"
}
