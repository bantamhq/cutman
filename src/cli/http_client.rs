use std::collections::HashMap;
use std::time::Duration;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::credentials::Credentials;
use crate::types::Namespace;

pub type NamespaceMap = HashMap<String, String>;

#[derive(Debug, Deserialize)]
pub struct NamespaceWithPrimary {
    #[serde(flatten)]
    pub namespace: Namespace,
    pub is_primary: bool,
}

#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
    token: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

impl ApiClient {
    pub fn new(creds: &Credentials) -> anyhow::Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;
        Ok(Self {
            client,
            base_url: creds.server_url.trim_end_matches('/').to_string(),
            token: creds.token.clone(),
        })
    }

    pub fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let url = format!("{}/api/v1{}", self.base_url, path);
        let resp = self.client.get(&url).bearer_auth(&self.token).send()?;
        self.handle_response(resp)
    }

    pub fn get_raw<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let url = format!("{}/api/v1{}", self.base_url, path);
        let resp = self.client.get(&url).bearer_auth(&self.token).send()?;
        if resp.status().is_success() {
            Ok(resp.json()?)
        } else {
            let api_resp: ApiResponse<()> = resp.json()?;
            Err(anyhow::anyhow!(api_resp.error.unwrap_or_else(|| {
                "Server error (no details provided)".into()
            })))
        }
    }

    pub fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        let url = format!("{}/api/v1{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(body)
            .send()?;
        self.handle_response(resp)
    }

    pub fn put<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        let url = format!("{}/api/v1{}", self.base_url, path);
        let resp = self
            .client
            .put(&url)
            .bearer_auth(&self.token)
            .json(body)
            .send()?;
        self.handle_response(resp)
    }

    pub fn patch<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        let url = format!("{}/api/v1{}", self.base_url, path);
        let resp = self
            .client
            .patch(&url)
            .bearer_auth(&self.token)
            .json(body)
            .send()?;
        self.handle_response(resp)
    }

    pub fn delete(&self, path: &str) -> anyhow::Result<()> {
        let url = format!("{}/api/v1{}", self.base_url, path);
        let resp = self.client.delete(&url).bearer_auth(&self.token).send()?;
        if resp.status().is_success() {
            Ok(())
        } else {
            let api_resp: ApiResponse<()> = resp.json()?;
            Err(anyhow::anyhow!(api_resp.error.unwrap_or_else(|| {
                "Server error (no details provided)".into()
            })))
        }
    }

    fn handle_response<T: DeserializeOwned>(
        &self,
        resp: reqwest::blocking::Response,
    ) -> anyhow::Result<T> {
        if resp.status().is_success() {
            let api_resp: ApiResponse<T> = resp.json()?;
            api_resp
                .data
                .ok_or_else(|| anyhow::anyhow!("Server returned an empty response"))
        } else {
            let api_resp: ApiResponse<()> = resp.json()?;
            Err(anyhow::anyhow!(api_resp.error.unwrap_or_else(|| {
                "Server error (no details provided)".into()
            })))
        }
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn fetch_namespace_map(&self) -> anyhow::Result<NamespaceMap> {
        let namespaces: Vec<NamespaceWithPrimary> = self.get("/namespaces")?;
        Ok(namespaces
            .into_iter()
            .map(|n| (n.namespace.id, n.namespace.name))
            .collect())
    }

    pub fn fetch_namespaces(&self) -> anyhow::Result<Vec<NamespaceWithPrimary>> {
        self.get("/namespaces")
    }
}
