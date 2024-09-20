pub mod cluster;
pub mod download;
pub mod workflow;

use crate::workflow::record::WorkflowRecordError;
use axum::{
    extract::{rejection::JsonRejection, FromRequest},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

// Create our own JSON extractor by wrapping `axum::Json`. This makes it easy to override the
// rejection and provide our own which formats errors to match our application.
//
// `axum::Json` responds with plain text if the input is invalid.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct AppJson<T>(T);

impl<T> IntoResponse for AppJson<T>
where
    axum::Json<T>: IntoResponse,
{
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

pub enum AppError {
    JsonRejection(JsonRejection),
    NotFoundError(anyhow::Error),
    TooManyRequests,
    InternalServerError(anyhow::Error),
}

// Tell axum how `AppError` should be converted into a response.
//
// This is also a convenient place to log errors.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // How we want errors responses to be serialized
        #[derive(Serialize)]
        struct ErrorResponse {
            message: String,
        }

        let (status, message) = match self {
            AppError::JsonRejection(rejection) => {
                // This error is caused by bad user input so don't log it
                (rejection.status(), rejection.body_text())
            }
            AppError::NotFoundError(error) => {
                (StatusCode::NOT_FOUND, format!("Not found: {}", error))
            }
            AppError::InternalServerError(error) => {
                eprintln!("Internal Server Error: {}", error);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error".to_string(),
                )
            }
            AppError::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                "No node available".to_string(),
            ),
        };

        (status, AppJson(ErrorResponse { message })).into_response()
    }
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        Self::JsonRejection(rejection)
    }
}

impl From<WorkflowRecordError> for AppError {
    fn from(error: WorkflowRecordError) -> Self {
        match error {
            WorkflowRecordError::PendingQueueFull => Self::TooManyRequests,
        }
    }
}

pub async fn health_check() -> &'static str {
    "ok"
}
