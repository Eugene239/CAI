use std::{error::Error, fmt, net::SocketAddr};

use axum::{Json, Router, routing::get};
use serde::Serialize;
use tokio::net::TcpListener;

pub fn router() -> Router {
    Router::new().route("/health", get(health))
}

pub async fn bind_loopback(address: SocketAddr) -> Result<TcpListener, ServerError> {
    if !address.ip().is_loopback() {
        return Err(ServerError(
            "CAI control-plane listener must use a loopback address".to_owned(),
        ));
    }

    TcpListener::bind(address).await.map_err(|error| {
        ServerError(format!(
            "could not bind CAI control-plane listener: {error}"
        ))
    })
}

pub async fn serve(listener: TcpListener) -> Result<(), ServerError> {
    axum::serve(listener, router())
        .await
        .map_err(|error| ServerError(format!("CAI control-plane server stopped: {error}")))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[derive(Debug)]
pub struct ServerError(String);

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ServerError {}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}
