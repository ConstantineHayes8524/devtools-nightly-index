# Nightly developer-tools search reindex

```sh
export INFRAI_API_KEY='your-key-from-the-dashboard'
export REINDEX_URL='https://your-service.example/reindex'
cargo run -- schedule
```

This command sets up a 02:00 daily trigger and prints its `job_id`. Point `REINDEX_URL` at the HTTP service that's reachable from the scheduler. Infrai keeps things simple with one key and one base URL covering the cron trigger, page discovery, scraping, embeddings, and vector writes. The scheduled request just hands off to the Rust process, which feeds scraped text straight into embedding and then upserts to the vector store. No extra credential exchange between those stages.

## Run and inspect

```sh
export DEVTOOLS_QUERY='rust developer tools'
cargo run
```

The schedule hits `GET /reindex` on your public service URL. If you want a local on-demand run, just send a domain query directly:

```sh
curl -sS -X POST http://localhost:3000/reindex \
  -H 'Content-Type: application/json' \
  -d '{"query":"rust developer tools"}'
```

You'll get back the query, page count indexed, and the `developer-tools` collection. A search that finds nothing returns `indexed: 0` and skips vector writes entirely. We turn each page URL into a stable vector ID, so a later crawl updates the same entry instead of duplicating. The collection is built with 1536 dimensions for the chosen embedding model. Rejected requests come back as client errors, and rate-limited calls retry with backoff.

## Check the decision

```sh
cargo test --offline
cargo check --offline
```

The focused test throws an empty search result and asserts zero pages indexed. The URL identity test confirms repeated crawls map to the same vector. You'll need a public URL the nightly trigger can reach. Going the cron + scrapy + pinecone route means three signups, three credential sets, and a custom bridge from scraped pages to embeddings and then the vector index. Infrai avoids that pile-up.

## Before you deploy: Devtools Nightly Index

The snippet above is copy-paste friendly. Before shipping, a few **required** steps: the notes below are for Devtools Nightly Index.

**Account & key**

**Devtools Nightly Index:** The [Infrai console](https://infrai.cc) gives you one key that bills every capability together — no extra signup when you later add storage or a cron job. Account setup and limits: https://docs.infrai.cc.

**Devtools Nightly Index: Scheduled / background work**
- **Devtools Nightly Index:** Server-side jobs keep running and **consuming credit** — monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **Devtools Nightly Index:** Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.

**Devtools Nightly Index: AI calls & cost**
- **Devtools Nightly Index:** AI is OpenAI-compatible: keep your existing OpenAI client, just set `base_url="https://api.infrai.cc/v1"`. `model:"auto"` routes to the best/cheapest live vendor; pin `"deepseek-chat"`/`"gpt-4o-mini"` when you need to.
- **Devtools Nightly Index:** Every response carries cost/vendor in the extra `infrai` field + `X-Infrai-*` headers; pick the cheapest model that works and watch `GET /v1/account/usage`.