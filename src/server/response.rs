use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::json;

use crate::error::Result as StoreResult;

/// Standard API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    #[must_use]
    pub fn success(data: T) -> Self {
        Self {
            data: Some(data),
            error: None,
        }
    }

    #[must_use]
    pub fn error(message: impl Into<String>) -> ApiResponse<()> {
        ApiResponse {
            data: None,
            error: Some(message.into()),
        }
    }
}

/// Paginated response for list endpoints
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub data: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

impl<T: Serialize> PaginatedResponse<T> {
    #[must_use]
    pub fn new(data: Vec<T>, next_cursor: Option<String>, has_more: bool) -> Self {
        Self {
            data,
            next_cursor,
            has_more,
        }
    }
}

/// API error that converts to a proper HTTP response
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    #[must_use]
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({ "data": null, "error": self.message });
        (self.status, Json(body)).into_response()
    }
}

/// Helper to paginate a slice and determine if there are more results
pub fn paginate<T, F>(items: Vec<T>, limit: usize, get_cursor: F) -> (Vec<T>, Option<String>, bool)
where
    F: Fn(&T) -> String,
{
    let has_more = items.len() > limit;
    let items: Vec<T> = items.into_iter().take(limit).collect();
    let next_cursor = if has_more {
        items.last().map(&get_cursor)
    } else {
        None
    };
    (items, next_cursor, has_more)
}

pub const DEFAULT_PAGE_SIZE: i32 = 50;

/// Extension trait for converting store results to API errors with a custom message.
pub trait StoreResultExt<T> {
    fn api_err(self, message: &'static str) -> Result<T, ApiError>;
}

impl<T> StoreResultExt<T> for StoreResult<T> {
    fn api_err(self, message: &'static str) -> Result<T, ApiError> {
        self.map_err(|_| ApiError::internal(message))
    }
}

/// Extension for Option types from store operations.
pub trait StoreOptionExt<T> {
    fn or_not_found(self, message: &'static str) -> Result<T, ApiError>;
}

impl<T> StoreOptionExt<T> for Option<T> {
    fn or_not_found(self, message: &'static str) -> Result<T, ApiError> {
        self.ok_or_else(|| ApiError::not_found(message))
    }
}
