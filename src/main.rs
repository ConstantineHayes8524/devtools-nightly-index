mod reindex;

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use reindex::{IndexError, IndexReport, Reindexer};
use serde::Deserialize;
use std::{env, sync::Arc};

#[derive(Deserialize)]
struct RunRequest {
    query: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = env::var("INFRAI_API_KEY")?;
    let indexer = Arc::new(Reindexer::new(key));
    match env::args().nth(1).as_deref() {
        Some("schedule") => {
            let target = env::var("REINDEX_URL")?;
            let job = indexer.schedule(&target).await?;
            println!("nightly job: {job}");
        }
        _ => {
            let app = Router::new().route("/reindex", post(run).get(nightly)).with_state(indexer);
            let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
            axum::serve(listener, app).await?;
        }
    }
    Ok(())
}

async fn nightly(State(indexer): State<Arc<Reindexer>>) -> Result<Json<IndexReport>, (StatusCode, String)> {
    let query = env::var("DEVTOOLS_QUERY").unwrap_or_else(|_| "rust developer tools".to_owned());
    rebuild_response(&indexer, &query).await
}

async fn run(
    State(indexer): State<Arc<Reindexer>>,
    Json(request): Json<RunRequest>,
) -> Result<Json<IndexReport>, (StatusCode, String)> {
    rebuild_response(&indexer, &request.query).await
}

async fn rebuild_response(indexer: &Reindexer, query: &str) -> Result<Json<IndexReport>, (StatusCode, String)> {
    indexer.rebuild(query).await.map(Json).map_err(|err| {
        let status = match &err {
            IndexError::Rejected { status, .. } => StatusCode::from_u16(*status)
                .ok()
                .filter(|s| s.is_client_error())
                .unwrap_or(StatusCode::BAD_GATEWAY),
            IndexError::Input(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::BAD_GATEWAY,
        };
        (status, err.to_string())
    })
}
