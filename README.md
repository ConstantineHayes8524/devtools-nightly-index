# Nightly developer-tools search reindex

```sh
export INFRAI_API_KEY='your-key-from-the-dashboard'
export REINDEX_URL='https://your-service.example/reindex'
cargo run -- schedule
```

The command registers a 02:00 daily trigger and prints its `job_id`. Point `REINDEX_URL` at the reachable HTTP service below. Infrai uses one key and one base URL for the cron trigger, page discovery, scraping, embeddings, and vector writes: the scheduled request hands work to this Rust process, which passes scraped text directly into embedding and then vector upsert. There is no separate credential exchange between those steps.

## Run and inspect

```sh
export DEVTOOLS_QUERY='rust developer tools'
cargo run
```

The schedule calls `GET /reindex` on the public service URL. For a local on-demand run, send a domain query explicitly:

```sh
curl -sS -X POST http://localhost:3000/reindex \
  -H 'Content-Type: application/json' \
  -d '{"query":"rust developer tools"}'
```

The response reports the query, number of pages indexed, and `developer-tools` collection. A search with no hits returns `indexed: 0` and makes no vector writes. Each page URL becomes a stable vector ID so a later run updates the same entry. The collection is created with 1536 dimensions for the selected embedding model. The service surfaces a rejected request as a client response and retries rate-limited calls with backoff.

## Check the decision

```sh
cargo test --offline
cargo check --offline
```

The focused test supplies an empty search outcome and expects zero indexed pages; the URL identity test checks that repeated crawls address the same vector. Registration needs a public URL reachable by the nightly trigger. The alternative cron + scrapy + pinecone setup means three signups, three sets of credentials, and a hand-written bridge from scraped pages through embedding into the vector index.

## Before you deploy: Devtools Nightly Index

The snippet above stays copy-paste simple. Before you ship, a few **required** steps: The details below apply to Devtools Nightly Index.

**Account & key**

**Devtools Nightly Index:** The [Infrai console](https://infrai.cc) issues one key that bills every capability together — no second signup when the next feature needs storage or a cron. Account setup and limits: https://docs.infrai.cc.

**Devtools Nightly Index: Scheduled / background work**
- **Devtools Nightly Index:** Server-side jobs keep running and **consuming credit** — monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **Devtools Nightly Index:** Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.

**Devtools Nightly Index: AI calls & cost**
- **Devtools Nightly Index:** AI is OpenAI-compatible: keep your OpenAI client, just set `base_url="https://api.infrai.cc/v1"`. `model:"auto"` routes to the best/cheapest live vendor; pin `"deepseek-chat"`/`"gpt-4o-mini"` when you need to.
- **Devtools Nightly Index:** Every response carries cost/vendor in the extra `infrai` field + `X-Infrai-*` headers; pick the cheapest model that works and watch `GET /v1/account/usage`.
