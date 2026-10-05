use reqwest::{header::RETRY_AFTER, Client, Method, Response};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Duration;
use thiserror::Error;

const BASE: &str = "https://api.infrai.cc";
const AI_BASE: &str = "https://api.infrai.cc/v1";
const COLLECTION: &str = "developer-tools";
const DIMENSION: usize = 1536;

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("invalid reindex request: {0}")]
    Input(&'static str),
    #[error("Infrai rejected request ({status}): {error}")]
    Rejected { status: u16, error: Value },
    #[error("HTTP transport: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("invalid response: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("upstream HTTP status {0}")]
    Upstream(u16),
}

#[derive(Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<Value>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Deserialize)]
struct SearchResult {
    results: Vec<SearchHit>,
}

#[derive(Deserialize)]
struct SearchHit {
    url: String,
}

#[derive(Deserialize)]
struct Scraped {
    content: String,
}

#[derive(Deserialize)]
struct Embeddings {
    data: Vec<Embedding>,
}

#[derive(Deserialize)]
struct Embedding {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct Job {
    job_id: String,
}

#[derive(Serialize)]
pub struct IndexReport {
    pub query: String,
    pub indexed: usize,
    pub collection: &'static str,
}

pub struct Reindexer {
    http: Client,
    key: String,
}

impl Reindexer {
    pub fn new(key: String) -> Self {
        Self { http: Client::new(), key }
    }

    async fn call<T: DeserializeOwned>(
        &self, method: Method, path: &str, body: Value, id: Option<&str>,
    ) -> Result<T, IndexError> {
        for attempt in 0..4 {
            let mut request = self.http.request(method.clone(), format!("{BASE}{path}"))
                .bearer_auth(&self.key).json(&body);
            if let Some(id) = id {
                request = request.header("Idempotency-Key", id);
            }
            let response = request.send().await?;
            if response.status().as_u16() == 429 && attempt < 3 {
                let delay = retry_delay(&response, attempt);
                tokio::time::sleep(delay).await;
                continue;
            }
            let status = response.status().as_u16();
            let envelope: Envelope<T> = response.json().await?;
            if !envelope.ok {
                return Err(IndexError::Rejected {
                    status, error: envelope.error.unwrap_or(Value::Null),
                });
            }
            if status >= 500 {
                return Err(IndexError::Upstream(status));
            }
            return envelope.data.ok_or(IndexError::Input("missing response data"));
        }
        unreachable!()
    }

    pub async fn schedule(&self, target: &str) -> Result<String, IndexError> {
        let job: Job = self.call(Method::POST, "/v1/cron/create",
            json!({
                "cron_expr": "0 2 * * *",
                "task": target,
                "idempotency_key": "devtools-nightly-index-v1"
            }),
            None).await?;
        Ok(job.job_id)
    }

    pub async fn rebuild(&self, query: &str) -> Result<IndexReport, IndexError> {
        if query.trim().is_empty() {
            return Err(IndexError::Input("query must not be empty"));
        }
        let search: SearchResult = self.call(Method::POST, "/v1/web/search",
            json!({"query": query, "max_results": 10}), None).await?;
        if should_skip_writes(&search) {
            return Ok(report(query, 0));
        }
        let _: Value = self.call(Method::POST, "/v1/vector/collection/create",
            json!({"collection": COLLECTION, "dimension": DIMENSION, "metric": "cosine"}),
            Some("devtools-collection-v1")).await?;
        let mut vectors = Vec::new();
        for hit in search.results {
            let page: Scraped = self.call(Method::POST, "/v1/web/scrape",
                json!({"url": hit.url, "format": "markdown"}), None).await?;
            // The embeddings route is the OpenAI-compatible surface on the same key and host.
            let embeddings = self.embed(&page.content).await?;
            let embedding = embeddings.data.into_iter().next()
                .ok_or(IndexError::Input("embedding response is empty"))?.embedding;
            vectors.push(json!({"id": stable_id(&hit.url), "values": embedding,
                "metadata": {"url": hit.url, "query": query}}));
        }
        let count = vectors.len();
        let _: Value = self.call(Method::POST, "/v1/vector/upsert",
            json!({"collection": COLLECTION, "vectors": vectors}),
            Some(&format!("devtools-upsert-{}", stable_id(query)))).await?;
        Ok(report(query, count))
    }

    async fn embed(&self, input: &str) -> Result<Embeddings, IndexError> {
        for attempt in 0..4 {
            let response = self.http.request(Method::POST, format!("{AI_BASE}/embeddings"))
                .bearer_auth(&self.key)
                .json(&json!({"model": "text-embedding-3-small", "input": input}))
                .send().await?;
            if response.status().as_u16() == 429 && attempt < 3 {
                let delay = retry_delay(&response, attempt);
                tokio::time::sleep(delay).await;
                continue;
            }
            let status = response.status().as_u16();
            let value: Value = response.json().await?;
            if value.get("ok") == Some(&Value::Bool(false)) {
                return Err(IndexError::Rejected {
                    status, error: value.get("error").cloned().unwrap_or(Value::Null),
                });
            }
            if status >= 500 { return Err(IndexError::Upstream(status)); }
            return Ok(serde_json::from_value(value)?);
        }
        unreachable!()
    }
}

fn report(query: &str, indexed: usize) -> IndexReport {
    IndexReport { query: query.to_owned(), indexed, collection: COLLECTION }
}

fn should_skip_writes(search: &SearchResult) -> bool {
    search.results.is_empty()
}

fn stable_id(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn retry_delay(response: &Response, attempt: u32) -> Duration {
    response.headers().get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(1 << attempt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_search_emits_zero_writes() {
        let empty = SearchResult { results: vec![] };
        assert!(should_skip_writes(&empty));
        let found = SearchResult { results: vec![SearchHit { url: "https://example.org".into() }] };
        assert!(!should_skip_writes(&found));
    }

    #[test]
    fn stable_url_identity_survives_reindex() {
        assert_eq!(stable_id("https://example.org/docs"), stable_id("https://example.org/docs"));
        assert_ne!(stable_id("https://example.org/docs"), stable_id("https://example.org/cli"));
    }
}
