use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    pub operation: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub transfers: Option<Vec<String>>,
    pub objects: Vec<ObjectSpec>,
}

#[derive(Debug, Serialize)]
pub struct BatchResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transfer: Option<String>,
    pub objects: Vec<ObjectResponse>,
}

#[derive(Debug, Deserialize)]
pub struct ObjectSpec {
    pub oid: String,
    pub size: i64,
}

#[derive(Debug, Serialize)]
pub struct ObjectResponse {
    pub oid: String,
    pub size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authenticated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<HashMap<String, Action>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ObjectError>,
}

impl ObjectResponse {
    #[must_use]
    pub fn with_error(oid: String, size: i64, code: i32, message: impl Into<String>) -> Self {
        Self {
            oid,
            size,
            authenticated: None,
            actions: None,
            error: Some(ObjectError {
                code,
                message: message.into(),
            }),
        }
    }

    #[must_use]
    pub fn with_actions(oid: String, size: i64, actions: HashMap<String, Action>) -> Self {
        Self {
            oid,
            size,
            authenticated: Some(true),
            actions: Some(actions),
            error: None,
        }
    }

    #[must_use]
    pub fn exists(oid: String, size: i64) -> Self {
        Self {
            oid,
            size,
            authenticated: Some(true),
            actions: None,
            error: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Action {
    pub href: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct ObjectError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct LfsError {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    pub oid: String,
    pub size: i64,
}
