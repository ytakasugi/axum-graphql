use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)] 
pub(crate) struct Health {
    pub healthy: bool
}

pub(crate) async fn health() -> impl IntoResponse {
    let health = Health {
        healthy: true
    };
    (StatusCode::OK, Json(health))
}