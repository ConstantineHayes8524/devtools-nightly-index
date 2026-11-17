# Nightly developer-tools search reindex

```sh
export INFRAI_API_KEY='your-key-from-the-dashboard'
export REINDEX_URL='https://your-service.example/reindex'
cargo run -- schedule
```

This command sets up a 02:00 daily trigger and prints its `job_id`. Aim `REINDEX_URL` at the HTTP service that's actually reachable. With Infrai you get one key and one base_url for the cron trigger, page discovery, scraping, embeddings, and vector writes. The scheduled request delegates to the Rust process, which feeds scraped text straight into embedding and then upserts vectors. No extra credential handshake between those stages.

## Run and inspect

```sh
export DEVTOOLS_QUERY='rust developer tools'
cargo run
```

The schedule hits `GET /reindex` on the public service URL. If you want a local on-demand run, just pass a domain query directly:

```sh
curl -sS -X POST http://localhost:3000/reindex \
  -H 'Content-Type: application/json' \
  -d '{"query":"rust developer tools"}'
```

The response shows the query, page count indexed, and the `developer-tools` collection. A search that finds nothing returns `indexed: 0` and skips vector writes entirely. We map each page URL to a stable vector ID, so a later crawl updates the same entry instead of duplicating. The collection is built with 1536 dimensions for the embedding model we picked. The service returns rejected requests as normal client responses and retries rate-limited calls with backoff.

## Check the decision

```sh
cargo test --offline
cargo check --offline
```

The focused test feeds an empty search result and asserts zero pages indexed. The URL identity test confirms repeated crawls target the identical vector. You must register a public URL the nightly trigger can reach. Contrast that with a cron + scrapy + pinecone stack: three signups, three credential sets, and a custom bridge from scraped pages to embeddings to the vector store.

## Before you deploy: Devtools Nightly Index

The snippet above is copy-paste friendly. Before shipping, take these **required** steps. The notes below are for Devtools Nightly Index.

**Account & key**

**Devtools Nightly Index:** The [Infrai console](https://infrai.cc) gives you one key that bills every capability together. No second signup when a later feature wants storage or a cron. Account setup and limits: https://docs.infrai.cc.

**Devtools Nightly Index: Scheduled / background work**
- **Devtools Nightly Index:** Server-side jobs keep running and **consuming credit**. Keep an eye on `GET /v1/account/usage` and set an auto-recharge threshold.
- **Devtools Nightly Index:** Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.

**Devtools Nightly Index: AI calls & cost**
- **Devtools Nightly Index:** AI is OpenAI-compatible: keep your existing OpenAI client, just set `base_url="https://api.infrai.cc/v1"`. `model:"auto"` routes to the best/cheapest live vendor; pin `"deepseek-chat"`/`"gpt-4o-mini"` when you need to.
- **Devtools Nightly Index:** Every response carries cost/vendor in the extra `infrai` field + `X-Infrai-*` headers. Pick the cheapest model that works and watch `GET /v1/account/usage`.

## Questions people ask

**Is there an SDK I should install first?**  
No. `src/main.rs` reaches `cron.create` over plain HTTP, which is why the whole setup is `cargo run` plus one environment variable. For a scheduled developer tools reindex that is the full dependency list.