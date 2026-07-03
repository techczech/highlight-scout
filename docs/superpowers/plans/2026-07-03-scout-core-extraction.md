# scout-core Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract Highlight Scout's FTS index + query grammar, Markdown+frontmatter archive IO, and content-hash idempotency utilities into a new `scout-core` repo, generalised over the record type, consumed back by highlight-scout as git dependencies — with highlight-scout behaviour byte-for-byte unchanged.

**Architecture:** `scout-core` is a monorepo: a Cargo workspace with two crates (`scout-index`, `scout-archive`) plus one TypeScript package (`@scout/query`, the search-query grammar) exposed from the repo root `package.json`. Generalisation is API-level naming only (`Work`→`Container`, `Highlight`→`Record`); the physical SQLite schema keeps its existing table/column names so shipped, R2-synced indexes need zero migration. Highlight Scout keeps a thin decoration layer that turns generic index hits into its Zotero-aware `SearchResult`.

**Tech Stack:** Rust 2021 (rusqlite `=0.31.0` bundled, anyhow, serde/serde_json, slug, sha1, regex — versions pinned to match highlight-scout), TypeScript + vitest, bun.

## Global Constraints

- Source baseline: highlight-scout **v0.5.5** (tag `v0.5.5`) at `~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout` — work on a branch `scout-core-extraction` there.
- New repo: `techczech/scout-core`, **private** (`gh repo create techczech/scout-core --private`), cloned at `~/gitrepos/06_apps-utilities/03_misc-utilities/scout-core`.
- Rust dep versions copied verbatim from highlight-scout `src-tauri/Cargo.toml`: `rusqlite = { version = "=0.31.0", features = ["bundled"] }`, `anyhow = "1.0.102"`, `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `regex = "1.12.4"`, `slug = "0.1.6"`, `sha1 = "0.11.0"`.
- **Physical schema is frozen**: table names `works`, `highlights`, `search_index` and all column names (incl. `work_id`, `work_type`, `highlighted_at`, `ocr_text`) stay exactly as in v0.5.5. Generic naming is Rust-API-only. Never rename tables/columns — the index syncs via R2 across machines.
- Highlight Scout behaviour parity is the acceptance bar: all pre-existing Rust + frontend tests pass unmodified in intent (moved tests run in scout-core; HS-side integration tests keep running in HS), and the serialized `SearchResult` contract to the HS frontend is unchanged field-for-field.
- Git hygiene: no `--no-verify`, no force-push, `Co-Authored-By` trailer on every commit, commit lockfiles.
- Decisions taken in Dominik's absence (flag in the final report, easy to revisit): monorepo grammar packaging; Container/Record API naming over trait-driven schema; repo home `03_misc-utilities`.

## File Structure

```
scout-core/                              (new repo, techczech/scout-core, private)
├── Cargo.toml                           workspace: members = crates/*
├── package.json                         npm package @scout/query at ROOT (git-dep subdir workaround)
├── tsconfig.json                        builds packages/scout-query/src → packages/scout-query/dist
├── README.md                            what scout-core is, who consumes it, schema-freeze warning
├── AGENTS.md                            telegraph agent instructions
├── .gitignore                           target/, node_modules/, packages/scout-query/dist/
├── .github/workflows/test.yml           cargo test + bun test
├── crates/
│   ├── scout-index/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                   pub mod models; pub mod sqlite;
│   │       ├── models.rs                Container, Record, Hit, SearchQuery, SearchPage, TagCount, Position, RegexFilter
│   │       └── sqlite.rs                generalised copy of HS index/sqlite.rs
│   └── scout-archive/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs                   pub mod markdown; pub mod idempotency;
│           ├── markdown.rs              generalised copy of HS import/archive.rs
│           └── idempotency.rs           generalised copy of HS import/common.rs
└── packages/scout-query/
    ├── src/
    │   ├── index.ts                     re-exports query + stopwords + types
    │   ├── query.ts                     moved from HS src/lib/query.ts
    │   ├── stopwords.ts                 moved from HS src/lib/stopwords.ts
    │   └── types.ts                     RegexFilter, SortMode, SearchMode (extracted subset of HS types.ts)
    └── query.test.ts                    moved from HS src/lib/query.test.ts

highlight-scout/  (modified)
├── src-tauri/Cargo.toml                 + scout-index, scout-archive git deps
├── src-tauri/src/models.rs              SearchQuery/RegexFilter/Work/Highlight/etc REMOVED (re-exported from scout-index); HS-only SearchResult + decorate() stay
├── src-tauri/src/index/{mod.rs,sqlite.rs}  DELETED (mod.rs) / DELETED (sqlite.rs)
├── src-tauri/src/import/archive.rs      DELETED — call sites use scout_archive::markdown
├── src-tauri/src/import/common.rs       DELETED — call sites use scout_archive::idempotency
├── src-tauri/src/commands/search.rs     payload→core-query translation + Hit→SearchResult decoration
├── src-tauri/src/{lib.rs,ocr.rs,qmd.rs,commands/import.rs,import/*.rs}  imports repointed
├── package.json                         + "@scout/query": "git+ssh://git@github.com/techczech/scout-core.git#v0.1.0"
├── src/lib/{query.ts,query.test.ts,stopwords.ts}  DELETED
└── src/{App.tsx,components/FilterPopover.tsx,lib/api.ts,lib/persist.ts,types.ts}  imports repointed to @scout/query
```

## Generalisation Rename Map (used by Tasks 2, 3, 5)

Rust API renames — apply **only** to Rust identifiers, never to SQL strings:

| highlight-scout (v0.5.5) | scout-core |
|---|---|
| `struct Work` | `struct Container` |
| `Work.work_type` | `Container.kind` (SQL column stays `work_type`) |
| `struct Highlight` | `struct Record` |
| `Highlight.work_id` | `Record.container_id` (SQL column stays `work_id`) |
| `Highlight.highlighted_at` | `Record.created_at` (SQL column stays `highlighted_at`) |
| `struct SearchResult` (index-level) | `struct Hit` (HS-specific fields removed — see Task 2) |
| `SearchQuery.work_type` | `SearchQuery.kind` |
| `SearchQuery.types` | `SearchQuery.kinds` |
| `SearchQuery.favorite: bool` | `SearchQuery.tag_any: Vec<String>` (OR of `h.tags LIKE '%…%'`) |
| `SearchQuery.zotero: bool` | `SearchQuery.source_any: Vec<String>` (OR of `w.source_system = ?`) |
| `upsert_work` | `upsert_container` |
| `upsert_highlight` | `upsert_record` |
| `work_highlights` | `container_records` |
| `highlight_position` | `record_position` |
| `highlight_by_id` | `record_by_id` |
| `work_id_by_slug` | `container_id_by_slug` |
| `all_works` / `all_highlights` | `all_containers` / `all_records` |
| `work_count` / `highlight_count` | `container_count` / `record_count` |
| `reindex_highlight_fts` | `reindex_record_fts` |
| `WorkPosition` | `Position` |
| `make_slug` | `make_slug` (unchanged) |
| `highlight_id(...)` (common.rs) | `record_id(...)` |
| `work_id(...)` (common.rs) | `container_id(...)` |

Unchanged in scout-index: `open`, `init_schema`, `search_query`, `list_tags`, `facets`, `ocr_sources`, `write_ocr`, `ocr_pending`, `escape_like`, `push_filters`, `build_order`, `compile_regexes`, `passes_negatives`, `passes_regexes`, `run_query`, `map_row` (signature changes — Task 2).

---

### Task 1: Scaffold the scout-core repo

**Files:**
- Create: `~/gitrepos/06_apps-utilities/03_misc-utilities/scout-core/{Cargo.toml,package.json,tsconfig.json,README.md,AGENTS.md,.gitignore,.github/workflows/test.yml}`
- Create: empty crate skeletons `crates/scout-index/{Cargo.toml,src/lib.rs}`, `crates/scout-archive/{Cargo.toml,src/lib.rs}`, `packages/scout-query/src/index.ts`

**Interfaces:**
- Produces: a building (empty) workspace + GitHub private repo `techczech/scout-core` with `main` pushed. Later tasks fill the crates/package.

- [x] **Step 1: Create local directory and git repo**

```bash
mkdir -p ~/gitrepos/06_apps-utilities/03_misc-utilities/scout-core
cd ~/gitrepos/06_apps-utilities/03_misc-utilities/scout-core
git init -b main
```

- [x] **Step 2: Write workspace + package scaffolding**

`Cargo.toml` (root):
```toml
[workspace]
resolver = "2"
members = ["crates/scout-index", "crates/scout-archive"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/techczech/scout-core"

[workspace.dependencies]
anyhow = "1.0.102"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "=0.31.0", features = ["bundled"] }
regex = "1.12.4"
slug = "0.1.6"
sha1 = "0.11.0"
```

`package.json` (root — this IS the `@scout/query` npm package; npm/bun git deps cannot target a subdirectory, so the TS package is exposed from the root with a `prepare` build):
```json
{
  "name": "@scout/query",
  "version": "0.1.0",
  "private": false,
  "type": "module",
  "main": "./packages/scout-query/dist/index.js",
  "types": "./packages/scout-query/dist/index.d.ts",
  "exports": {
    ".": {
      "types": "./packages/scout-query/dist/index.d.ts",
      "default": "./packages/scout-query/dist/index.js"
    }
  },
  "files": ["packages/scout-query/dist", "packages/scout-query/src"],
  "scripts": {
    "build": "tsc -p tsconfig.json",
    "prepare": "tsc -p tsconfig.json",
    "test": "vitest run"
  },
  "devDependencies": {
    "typescript": "~5.6.2",
    "vitest": "^2.1.4"
  }
}
```

`tsconfig.json` (root):
```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "declaration": true,
    "outDir": "packages/scout-query/dist",
    "rootDir": "packages/scout-query/src",
    "skipLibCheck": true
  },
  "include": ["packages/scout-query/src"]
}
```

`crates/scout-index/Cargo.toml`:
```toml
[package]
name = "scout-index"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Generic FTS5 index + search over container/record archives (Scout family)"

[dependencies]
anyhow.workspace = true
serde.workspace = true
serde_json.workspace = true
rusqlite.workspace = true
regex.workspace = true
```

`crates/scout-archive/Cargo.toml`:
```toml
[package]
name = "scout-archive"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Markdown+frontmatter archive IO and content-hash idempotency (Scout family)"

[dependencies]
anyhow.workspace = true
serde_json.workspace = true
slug.workspace = true
sha1.workspace = true

[dev-dependencies]
serde.workspace = true
```

`crates/scout-index/src/lib.rs` and `crates/scout-archive/src/lib.rs` (placeholders replaced in Tasks 2–3):
```rust
// scout-index: filled in Task 2
```
```rust
// scout-archive: filled in Task 3
```

`packages/scout-query/src/index.ts`:
```ts
// filled in Task 4
export {};
```

`.gitignore`:
```
target/
node_modules/
packages/scout-query/dist/
```

`.github/workflows/test.yml`:
```yaml
name: test
on: [push, pull_request]
jobs:
  rust:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --workspace
  ts:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: oven-sh/setup-bun@v2
      - run: bun install
      - run: bun run test
```

`README.md` — FOR YOU register, short: what scout-core is (shared engine of Highlight Scout / ArchiveScout / later SlideWell), the three parts, the **schema-freeze rule** (physical names `works`/`highlights`/`search_index` are frozen; generic naming is API-level only), and the root-package.json quirk. `AGENTS.md` — FOR ME register: consumers, schema freeze, "never rename SQL identifiers", test commands.

- [x] **Step 3: Verify the empty workspace builds and commit**

```bash
cargo build --workspace   # expect: success (2 empty crates)
bun install               # expect: lockfile created
git add -A && git commit -m "chore: scaffold scout-core workspace (scout-index, scout-archive, @scout/query)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

- [x] **Step 4: Create the private GitHub repo and push**

```bash
gh repo create techczech/scout-core --private --source . --push
```
Expected: repo exists, `main` pushed.

---

### Task 2: crates/scout-index — generalised index + search

**Files:**
- Create: `crates/scout-index/src/models.rs`, replace `crates/scout-index/src/lib.rs`, create `crates/scout-index/src/sqlite.rs`
- Source material: highlight-scout `src-tauri/src/models.rs` (lines 1–133) and `src-tauri/src/index/sqlite.rs` (lines 1–905) at tag v0.5.5

**Interfaces:**
- Produces (consumed by Task 5):
  - `scout_index::models::{Container, Record, Hit, SearchQuery, SearchPage, TagCount, Position, RegexFilter}`
  - `scout_index::sqlite::{open, init_schema, upsert_container, upsert_record, reindex_record_fts, ocr_sources, write_ocr, ocr_pending, search_query, container_records, record_position, record_by_id, container_id_by_slug, list_tags, facets, all_containers, all_records, record_count, container_count}`
  - Signature changes vs HS: `map_row`/`run_query`/`search_query`/`container_records`/`record_by_id` **lose the `archive: &str` parameter** (asset-path derivation moves to HS). `search_query(conn, &SearchQuery) -> Result<SearchPage>`; `SearchPage { rows: Vec<Hit>, has_more: bool }`.

- [x] **Step 1: Write models.rs**

Copy HS `models.rs`, apply the rename map, drop HS-only types (`SearchResult`, `ImportStatus` stay in HS). New `Hit` replaces index-level `SearchResult`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub author: Option<String>,
    /// Container kind ("article", "book", …). SQL column: work_type.
    pub kind: String,
    pub source_system: String,
    pub source_id: Option<String>,
    pub url: Option<String>,
    pub imported_at: String,
    pub updated_at: String,
    pub source_data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    /// SQL column: work_id.
    pub container_id: String,
    pub text: String,
    pub note: Option<String>,
    /// SQL column: highlighted_at.
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub tags: Vec<String>,
    pub location: Option<String>,
    pub location_type: Option<String>,
    pub annotation_color: Option<String>,
    pub annotation_type: Option<String>,
    pub format: String,
    pub source_data: serde_json::Value,
}

/// One search hit: the denormalised record+container row, app-agnostic.
/// Apps decorate this into their own result types (e.g. Highlight Scout adds
/// citation/zotero_link/asset_path derived from source_data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub record_id: String,
    pub container_id: String,
    pub slug: String,
    pub text: String,
    pub note: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub kind: String,
    pub source_system: String,
    pub source_id: Option<String>,
    pub url: Option<String>,
    pub created_at: Option<String>,
    pub tags: Vec<String>,
    pub location: Option<String>,
    pub annotation_color: Option<String>,
    pub annotation_type: Option<String>,
    pub format: String,
    pub ocr_text: Option<String>,
    pub container_source_data: serde_json::Value,
    pub record_source_data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPage {
    pub rows: Vec<Hit>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexFilter {
    pub source: String,
    pub flags: String,
}

/// Structured query. Apps parse their query grammar (e.g. @scout/query) and
/// translate app-level toggles into the generic fields:
/// favorite → tag_any, zotero → source_any.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchQuery {
    pub fts: String,
    pub has_positive: bool,
    #[serde(default)]
    pub positive_terms: Vec<String>,
    #[serde(default)]
    pub negatives: Vec<String>,
    #[serde(default)]
    pub regexes: Vec<RegexFilter>,
    pub author: Option<String>,
    pub title: Option<String>,
    pub kind: Option<String>,
    pub tag: Option<String>,
    /// OR across tags-LIKE clauses (was HS `favorite`).
    #[serde(default)]
    pub tag_any: Vec<String>,
    /// OR across exact source_system matches (was HS `zotero`).
    #[serde(default)]
    pub source_any: Vec<String>,
    #[serde(default)]
    pub has_image: bool,
    /// Combinable kind filter (OR across the list). Empty = no filter.
    #[serde(default)]
    pub kinds: Vec<String>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub source: Option<String>,
    pub color: Option<String>,
    pub sort: String,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub pos: i64,
    pub total: i64,
    pub max_loc: i64,
}
```

`lib.rs`:
```rust
pub mod models;
pub mod sqlite;
```

- [x] **Step 2: Write sqlite.rs — copy + mechanical transform**

Copy HS `src-tauri/src/index/sqlite.rs` lines 1–785 (everything above `#[cfg(test)]`) into `crates/scout-index/src/sqlite.rs`, then apply exactly these transformations:

1. `use crate::models::{…}` → `use crate::models::{Container, Hit, Position, Record, SearchPage, SearchQuery, TagCount};` and `crate::models::RegexFilter` in `compile_regexes`.
2. Apply the Generalisation Rename Map to all Rust identifiers, struct literals, and field accesses. **Do not touch any SQL string** — all `works`/`highlights`/`work_id`/`work_type`/`highlighted_at` column references stay verbatim.
3. Struct-field ↔ column mapping inside functions: `work.kind` binds to the `work_type` column in `upsert_container`; `h.container_id` → column `work_id` in `upsert_record`; `h.created_at` → column `highlighted_at`.
4. `map_row(row: &rusqlite::Row) -> rusqlite::Result<Hit>` — drop the `archive: &str` param; delete the `asset_path`, `citation`, `authors`, `collections`, `zotero_link` derivation block (HS reclaims it in Task 5); instead parse columns 17/18 into `container_source_data` / `record_source_data` `serde_json::Value`s and populate `Hit` (there is no `relevance`/`snippet` on `Hit`).
5. `push_filters`: replace the `favorite` and `zotero` blocks with:
```rust
if !q.tag_any.is_empty() {
    let ors: Vec<String> = q.tag_any.iter().map(|t| {
        params.push(Box::new(format!("%{}%", t)));
        format!("h.tags LIKE ?{}", params.len())
    }).collect();
    add(format!("({})", ors.join(" OR ")));
}
// … (has_image, kinds blocks unchanged apart from renames) …
if !q.source_any.is_empty() {
    let ors: Vec<String> = q.source_any.iter().map(|s| {
        params.push(Box::new(s.to_string()));
        format!("w.source_system = ?{}", params.len())
    }).collect();
    add(format!("({})", ors.join(" OR ")));
}
```
Keep clause ordering identical to v0.5.5 (`author, title, kind, tag, tag_any, source_any, has_image, kinds, source, color, after, before`) so generated SQL parameter numbering is deterministic.
6. `run_query`, `search_query`, `container_records`, `record_by_id` lose the `archive` param; `passes_negatives`/`passes_regexes` operate on `Hit` (field `r.text`, `r.title`, `r.author`, `r.note` — unchanged names).

- [x] **Step 3: Port the in-memory tests + add parity tests**

In `#[cfg(test)] mod tests` (same file), port from HS the two self-contained tests (`migrates_old_search_index_to_include_ocr`, `search_matches_text_found_only_in_ocr`) with renames, and the `keyword_query` helper (fields updated: `kind: None, tag_any: vec![], source_any: vec![], kinds: vec![]`). The Zotero-DB integration test does NOT move (stays in HS, Task 5). Add two new generalisation-parity tests:

```rust
#[test]
fn tag_any_matches_favorite_semantics() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    conn.execute("INSERT INTO works (id,slug,title,author,work_type,source_system,source_id,url,imported_at,updated_at,source_data) VALUES ('w1','w1','W',NULL,'article','x',NULL,NULL,'t','t','{}')", []).unwrap();
    conn.execute("INSERT INTO highlights (id,work_id,text,tags,format,source_data) VALUES ('h1','w1','alpha','[\"favorite\"]','plain','{}')", []).unwrap();
    conn.execute("INSERT INTO highlights (id,work_id,text,tags,format,source_data) VALUES ('h2','w1','alpha','[]','plain','{}')", []).unwrap();
    reindex_record_fts(&conn, "h1").unwrap();
    reindex_record_fts(&conn, "h2").unwrap();
    let mut q = keyword_query("alpha", None);
    q.tag_any = vec!["favorite".into(), "Liked".into()];
    let page = search_query(&conn, &q).unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].record_id, "h1");
}

#[test]
fn source_any_matches_source_system_exactly() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    conn.execute("INSERT INTO works (id,slug,title,author,work_type,source_system,source_id,url,imported_at,updated_at,source_data) VALUES ('w1','w1','W',NULL,'article','zotero',NULL,NULL,'t','t','{}')", []).unwrap();
    conn.execute("INSERT INTO works (id,slug,title,author,work_type,source_system,source_id,url,imported_at,updated_at,source_data) VALUES ('w2','w2','W2',NULL,'article','readwise',NULL,NULL,'t','t','{}')", []).unwrap();
    conn.execute("INSERT INTO highlights (id,work_id,text,tags,format,source_data) VALUES ('h1','w1','beta','[]','plain','{}')", []).unwrap();
    conn.execute("INSERT INTO highlights (id,work_id,text,tags,format,source_data) VALUES ('h2','w2','beta','[]','plain','{}')", []).unwrap();
    reindex_record_fts(&conn, "h1").unwrap();
    reindex_record_fts(&conn, "h2").unwrap();
    let mut q = keyword_query("beta", None);
    q.source_any = vec!["zotero".into()];
    let page = search_query(&conn, &q).unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].source_system, "zotero");
}
```

Note: `reindex_record_fts` on rows inserted directly is what populates `search_index` here — do not insert into `search_index` by hand.

- [x] **Step 4: Run tests**

```bash
cd ~/gitrepos/06_apps-utilities/03_misc-utilities/scout-core
cargo test -p scout-index
```
Expected: PASS — 4 tests (2 ported + 2 new).

- [x] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(scout-index): generalised FTS index + search extracted from highlight-scout v0.5.5

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: crates/scout-archive — Markdown archive IO + idempotency hashes

**Files:**
- Create: `crates/scout-archive/src/markdown.rs`, `crates/scout-archive/src/idempotency.rs`, replace `crates/scout-archive/src/lib.rs`
- Source material: HS `src-tauri/src/import/archive.rs` (lines 1–251), `src-tauri/src/import/common.rs` (lines 1–56)

**Interfaces:**
- Produces (consumed by Task 5):
  - `scout_archive::markdown::{make_slug, write_fulltext, write_import_batch, write_archive, render_container_file, render_record, yaml_escape}` — same signatures as HS, with `&[Work]`→`&[scout_index::models::Container]` and `&Highlight`→`&Record`. To avoid a circular dep, scout-archive does **not** depend on scout-index: it defines the function signatures generically over the two structs via a local minimal trait pair:

```rust
/// Minimal read-view of a container for archive rendering.
pub trait ContainerMeta {
    fn slug(&self) -> &str;
    fn id(&self) -> &str;
    fn title(&self) -> &str;
    fn author(&self) -> Option<&str>;
    fn kind(&self) -> &str;
    fn source_system(&self) -> &str;
    fn source_id(&self) -> Option<&str>;
    fn url(&self) -> Option<&str>;
    fn imported_at(&self) -> &str;
    fn updated_at(&self) -> &str;
    fn source_data_json(&self) -> String;
}

/// Minimal read-view of a record for archive rendering.
pub trait RecordMeta {
    fn id(&self) -> &str;
    fn text(&self) -> &str;
    fn note(&self) -> Option<&str>;
    fn created_at(&self) -> Option<&str>;
    fn tags(&self) -> &[String];
    fn annotation_color(&self) -> Option<&str>;
    fn annotation_type(&self) -> Option<&str>;
    fn format(&self) -> &str;
}
```
  and `write_archive<C: ContainerMeta, R: RecordMeta>(archive_path: &str, containers: &[C], records_by_container: &HashMap<String, Vec<&R>>) -> Result<()>`, `render_container_file<C, R>(…) -> String`, `render_record<R: RecordMeta>(r: &R) -> String` (now `pub`). Task 5 implements both traits for `scout_index::models::{Container, Record}` inside highlight-scout (a ~30-line impl block).
  - `scout_archive::idempotency::{record_id, container_id}` — bodies identical to HS `highlight_id`/`work_id` (SHA1, `\x1f` separator, same prefix format `{source}-…` / `{source}-w-…` so existing archives re-import idempotently).

- [x] **Step 1: Write idempotency.rs** — copy HS `common.rs` verbatim, rename `highlight_id`→`record_id`, `work_id`→`container_id`, update the module doc comment (drop the ADR-0011 cross-reference, state the guarantee inline), keep all three tests (renamed calls).

- [x] **Step 2: Write markdown.rs** — copy HS `archive.rs`; convert `render_work_file`/`render_highlight`/`write_archive` to the trait-generic forms above (`work.title` → `work.title()`, etc.; frontmatter emission order and every literal byte of output unchanged); make `render_record` and `yaml_escape` (renamed from `escape_yaml`) `pub`. Port all 7 tests, adding a local `struct TestRecord`/`impl RecordMeta` test double replicating the old `sample_highlight` values.

- [x] **Step 3: Run tests**

```bash
cargo test -p scout-archive
```
Expected: PASS — 10 tests (3 idempotency + 7 markdown).

- [x] **Step 4: Commit**

```bash
git add -A && git commit -m "feat(scout-archive): markdown archive IO + content-hash idempotency extracted from highlight-scout v0.5.5

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: packages/scout-query — the query grammar

**Files:**
- Create: `packages/scout-query/src/{query.ts,stopwords.ts,types.ts,index.ts}`, `packages/scout-query/query.test.ts`
- Source material: HS `src/lib/query.ts` (346 lines), `src/lib/stopwords.ts` (25 lines), `src/lib/query.test.ts` (59 lines), plus `RegexFilter`/`SearchMode`/`SortMode` from HS `src/types.ts:34,115,116`

**Interfaces:**
- Produces (consumed by Task 6): package `@scout/query` exporting everything HS currently imports from `./lib/query` — `parseSearch`, `buildSearchQuery`, `filtersActive`, `EMPTY_FILTERS`, plus types `ParsedQuery`, `SearchQueryPayload`, `Filters`, `RegexFilter`, `SortMode`, `SearchMode`, and `isStopword`. **The `SearchQueryPayload` wire shape is unchanged** (still `favorite`/`zotero`/`types` etc.) — translation to the generic Rust `SearchQuery` happens in HS's Rust command layer (Task 5), not in TS.

- [ ] **Step 1: Move the three source files** — copy `query.ts` and `stopwords.ts` verbatim; create `types.ts` containing exactly the `RegexFilter` interface and `SearchMode`/`SortMode` type aliases copied from HS `src/types.ts`; fix `query.ts`'s import to `from "./types"`. Write `index.ts`:

```ts
export * from "./query";
export * from "./types";
export { STOPWORDS, isStopword, withoutStopwords } from "./stopwords";
```
(These are the three names `stopwords.ts` exports — verified against v0.5.5.)

- [ ] **Step 2: Move the test file** — copy `query.test.ts`, fix its import path to `./src/query`.

- [ ] **Step 3: Build + run tests**

```bash
bun install && bun run build && bun run test
```
Expected: tsc emits `packages/scout-query/dist/`; vitest PASS (same count as HS's query.test.ts had).

- [ ] **Step 4: Commit, tag v0.1.0, push**

```bash
git add -A && git commit -m "feat(scout-query): search-query grammar extracted from highlight-scout v0.5.5

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git tag v0.1.0 && git push -u origin main --tags
```

---

### Task 5: highlight-scout Rust switchover

**Files:**
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/models.rs`, `src-tauri/src/commands/search.rs`, `src-tauri/src/commands/import.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/ocr.rs`, `src-tauri/src/qmd.rs`, `src-tauri/src/import/{mod.rs,csv_import.rs,kindle.rs,readwise.rs,readwise_seed.rs,readwise_tweets.rs,tweet_common.rs,x.rs,zotero.rs}`
- Delete: `src-tauri/src/index/` (both files), `src-tauri/src/import/archive.rs`, `src-tauri/src/import/common.rs`
- Branch: `scout-core-extraction` in highlight-scout

**Interfaces:**
- Consumes: everything Tasks 2–3 produce, via `scout-index = { git = "ssh://git@github.com/techczech/scout-core.git", tag = "v0.1.0" }` (same for `scout-archive`).
- Produces: HS `models.rs` keeps ONLY `SearchResult`, `ImportStatus`, and new `pub fn decorate(hit: scout_index::models::Hit, archive: &str) -> SearchResult` + `pub struct SearchPayload` (the old HS `SearchQuery` deserialization shape, unchanged serde attrs incl. `#[serde(rename = "type")]`) + `pub fn to_core_query(p: SearchPayload) -> scout_index::models::SearchQuery`; re-exports `pub use scout_index::models::{Container as Work, Record as Highlight, TagCount, Position as WorkPosition};` so importers keep compiling with minimal churn.

- [ ] **Step 1: Add git deps + delete moved modules.** In `src-tauri/Cargo.toml` `[dependencies]` add the two git deps above. Delete the four files; remove `pub mod index;` from `lib.rs` module list and `pub mod archive; pub mod common;` from `import/mod.rs`. Add `[net] git-fetch-with-cli = true` note: if `cargo build` cannot auth, create `src-tauri/.cargo/config.toml`… — no: put it in the repo root `.cargo/config.toml` with exactly:

```toml
[net]
git-fetch-with-cli = true
```

- [ ] **Step 2: Rebuild models.rs.** Keep `SearchResult` (all fields incl. `relevance`, `snippet`) and `ImportStatus`. Delete `Work`, `Highlight`, `SearchPage`, `RegexFilter`, `SearchQuery`, `TagCount`, `WorkPosition` and replace with re-exports (above). Add:

```rust
/// Turn a generic index Hit into the HS SearchResult the frontend expects.
/// Reclaims the derivation dropped from scout-core's map_row: asset_path,
/// citation, authors, collections, zotero_link (from source_data JSON).
pub fn decorate(hit: scout_index::models::Hit, archive: &str) -> SearchResult {
    let asset_path = if hit.format == "image" {
        Some(format!("{}/readings/assets/{}.png", archive.trim_end_matches('/'), hit.record_id))
    } else { None };
    let work_sd = &hit.container_source_data;
    let hl_sd = &hit.record_source_data;
    let citation = work_sd.get("citation").and_then(|v| v.as_str())
        .filter(|s| !s.is_empty()).map(String::from);
    let authors: Vec<String> = work_sd.get("authors").and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let collections: Vec<String> = work_sd.get("collections").and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let zotero_link = match (
        hl_sd.get("zotero_attachment_key").and_then(|v| v.as_str()),
        hl_sd.get("zotero_annotation_key").and_then(|v| v.as_str()),
    ) {
        (Some(ak), Some(annk)) if !ak.is_empty() =>
            Some(format!("zotero://open-pdf/library/items/{}?annotation={}", ak, annk)),
        (Some(ak), _) if !ak.is_empty() =>
            Some(format!("zotero://open-pdf/library/items/{}", ak)),
        _ => None,
    };
    SearchResult {
        highlight_id: hit.record_id, work_id: hit.container_id, slug: hit.slug,
        text: hit.text, note: hit.note, title: hit.title, author: hit.author,
        authors, work_type: hit.kind, source_system: hit.source_system,
        source_id: hit.source_id, url: hit.url, highlighted_at: hit.created_at,
        tags: hit.tags, location: hit.location, annotation_color: hit.annotation_color,
        annotation_type: hit.annotation_type, format: hit.format, asset_path,
        citation, collections, zotero_link, ocr_text: hit.ocr_text,
        relevance: None, snippet: String::new(),
    }
}
```

`SearchPayload`: copy the old HS `SearchQuery` struct verbatim (rename only the struct), and:

```rust
pub fn to_core_query(p: SearchPayload) -> scout_index::models::SearchQuery {
    scout_index::models::SearchQuery {
        fts: p.fts, has_positive: p.has_positive, positive_terms: p.positive_terms,
        negatives: p.negatives,
        regexes: p.regexes.into_iter()
            .map(|r| scout_index::models::RegexFilter { source: r.source, flags: r.flags })
            .collect(),
        author: p.author, title: p.title, kind: p.work_type, tag: p.tag,
        tag_any: if p.favorite { vec!["favorite".into(), "Liked".into()] } else { vec![] },
        source_any: if p.zotero { vec!["zotero".into()] } else { vec![] },
        has_image: p.has_image, kinds: p.types,
        after: p.after, before: p.before, source: p.source, color: p.color,
        sort: p.sort, page: p.page, page_size: p.page_size,
    }
}
```
(If the re-exported `RegexFilter` is the same type, the `.map` collapses to a direct move — prefer that: HS `SearchPayload.regexes` should be typed as `Vec<scout_index::models::RegexFilter>` so no conversion is needed.)

- [ ] **Step 3: Repoint call sites.** Mechanical, guided by the grep inventory:
  - `commands/search.rs`: `use scout_index::sqlite;` — Tauri commands keep their external names/signatures; internally: deserialize `SearchPayload`, `let q = to_core_query(payload);`, `sqlite::search_query(&conn, &q)` then `page.rows.into_iter().map(|h| decorate(h, &archive)).collect()` into the old `SearchPage`-shaped response (define a local `#[derive(Serialize)] struct ResultPage { rows: Vec<SearchResult>, has_more: bool }` to keep the wire shape identical). Same decoration for `work_highlights`→`container_records`, `highlight_by_id`→`record_by_id`; counts/tags/facets/position are direct renames.
  - `commands/import.rs`, `lib.rs`: `sqlite::upsert_work`→`scout_index::sqlite::upsert_container`, `upsert_highlight`→`upsert_record`, `all_works`→`all_containers`, `all_highlights`→`all_records`; `import::archive::write_archive`→`scout_archive::markdown::write_archive`, `write_fulltext`, `write_import_batch` likewise.
  - Importers (`csv_import.rs`, `kindle.rs`, `readwise*.rs`, `tweet_common.rs`, `x.rs`, `zotero.rs`): `use crate::import::archive::make_slug;`→`use scout_archive::markdown::make_slug;`, `use crate::import::common::{highlight_id, work_id};`→`use scout_archive::idempotency::{record_id as highlight_id, container_id as work_id};` (aliased to avoid touching bodies). Struct literals: because of the `models.rs` re-export aliases, `Work{…}`/`Highlight{…}` literals need field renames only where fields changed: `work_type:`→`kind:`, `work_id:`→`container_id:`, `highlighted_at:`→`created_at:` — do these with careful per-file edits, not blind sed.
  - `ocr.rs`, `qmd.rs`: direct renames per the map.
  - Add the `ContainerMeta`/`RecordMeta` trait impls for the re-exported `Container`/`Record` in a new small `src-tauri/src/archive_meta.rs` (registered in `lib.rs`), delegating each method to the corresponding field (`fn kind(&self) -> &str { &self.kind }`, `fn source_data_json(&self) -> String { serde_json::to_string(&self.source_data).unwrap_or_else(|_| "{}".into()) }`).

- [ ] **Step 4: Move the Zotero integration test.** The `full_zotero_pipeline_indexes_and_searches` test (deleted with `index/sqlite.rs`) is recreated in HS at `src-tauri/src/import/zotero.rs`'s test module (or a new `src-tauri/tests/zotero_pipeline.rs` integration test) using `scout_index::sqlite::*` + `decorate` — same assertions, renamed calls, `search_query(&conn, &q)` without archive param, colour filter assertion via `decorate`d results.

- [ ] **Step 5: Full test run + build**

```bash
cd ~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout/src-tauri
cargo test
cargo build
```
Expected: PASS. Test count = 41 minus the 12 moved to scout-core (2 index in-memory + 7 archive + 3 common) plus the re-homed Zotero pipeline test — verify the arithmetic against the actual run and record the new number.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "refactor: consume scout-index + scout-archive from scout-core (Rust extraction)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: highlight-scout frontend switchover

**Files:**
- Modify: `package.json`, `src/App.tsx:38`, `src/components/FilterPopover.tsx:2`, `src/lib/api.ts:15`, `src/lib/persist.ts:5`, `src/types.ts`
- Delete: `src/lib/query.ts`, `src/lib/query.test.ts`, `src/lib/stopwords.ts`

**Interfaces:**
- Consumes: `@scout/query` v0.1.0 (Task 4). Import specifier changes only; no call-shape changes.

- [ ] **Step 1: Add dependency + delete moved files**

```bash
cd ~/gitrepos/06_apps-utilities/01_desktop-apps/highlight-scout
bun add "@scout/query@git+ssh://git@github.com/techczech/scout-core.git#v0.1.0"
rm src/lib/query.ts src/lib/query.test.ts src/lib/stopwords.ts
```
(Verify bun ran the `prepare` script — `ls node_modules/@scout/query/packages/scout-query/dist/` must show `index.js` + `index.d.ts`. If bun skipped prepare, commit `dist/` in scout-core instead and re-tag — note which path was taken.)

- [ ] **Step 2: Repoint imports.** In the four files, change `from "./lib/query"` / `from "../lib/query"` / `from "./query"` to `from "@scout/query"`. In `src/types.ts`, replace the local `RegexFilter`/`SearchMode`/`SortMode` definitions with `export type { RegexFilter, SearchMode, SortMode } from "@scout/query";` so the rest of the frontend is untouched. Check `stopwords` had no other importers (`grep -rn "stopwords" src/`).

- [ ] **Step 3: Frontend tests + typecheck + build**

```bash
bun run test        # vitest — expect PASS, count = 29 minus the moved query tests
bunx tsc --noEmit   # expect clean
bun run build       # expect vite build success
```

- [ ] **Step 4: Live verification** — launch the app (`bun run tauri dev`), run one keyword search, one filtered search (favourite toggle + a type filter + a `-negative`), open a work view, confirm a Zotero result still shows citation + zotero link. This exercises decorate() and the payload translation end-to-end.

- [ ] **Step 5: Commit and merge**

```bash
git add -A && git commit -m "refactor: consume @scout/query from scout-core (frontend extraction)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git checkout main && git merge --no-ff scout-core-extraction && git push
```
No release/tag: dependency swap with behaviour parity; next feature release picks it up.

---

### Task 7: Coordination + registry updates

**Files:**
- Create: `~/gitrepos/_COORDINATION/highlights/_TASK-LOG/2026-07-03-scout-core-extraction.md` (or dated when executed)
- Create: `~/gitrepos/_COORDINATION/highlights/_ADR/0015-scout-core-extraction.md` (0001–0014 exist; re-verify at execution time)
- Modify: `~/gitrepos/_COORDINATION/highlights/AGENTS.md` (add scout-core to Repos + Routing), `~/gitrepos/_REPOLOG/` registry (follow its README for the registration procedure)

**Interfaces:**
- Consumes: outcomes of Tasks 1–6 (repo URL, tag, test counts).

- [ ] **Step 1: Write the ADR** — decisions: (a) monorepo crates+TS-package with root-package.json git-dep workaround; (b) API-level generalisation, physical schema frozen; (c) `tag_any`/`source_any` replacing `favorite`/`zotero` in the core query with HS-side translation; (d) repo home `03_misc-utilities`. Include the trade-offs actually weighed (trait-driven schema rejected as premature; Rust port of grammar rejected as rewrite risk). Verify it meets the three ADR criteria (hard to reverse, surprising without context, real trade-off).
- [ ] **Step 2: Write the task-log record** — outcome, test counts before/after, live-verification note, link to this plan.
- [ ] **Step 3: Update highlights AGENTS.md routing + register in _REPOLOG**; push `_COORDINATION` and `_REPOLOG` same turn.
- [ ] **Step 4: Commit each repo, clean trees everywhere.**

---

## Self-Review (done at authoring time)

- **Spec coverage:** FTS index builder ✓ (Task 2), query grammar ✓ (Task 4 — stays TS by decision), archive IO ✓ (Task 3), hash/idempotency ✓ (Task 3), generic over record type ✓ (Container/Record + Hit), own repo ✓ (Task 1), git dependency ✓ (Tasks 5–6), inherited+generalised test suite ✓ (Tasks 2–4 move tests; Task 5 re-homes the Zotero integration test). ArchiveScout consumption is out of scope here (its own build follows the design-first process).
- **Known risk flags:** bun `prepare` on git deps (fallback documented in Task 6 Step 1); struct-literal field renames in importers must be hand-checked (Task 5 Step 3 says no blind sed); SQL parameter-numbering determinism preserved by keeping clause order (Task 2 Step 2.5).
- **Type consistency check:** `Hit` fields ↔ `decorate()` ↔ `SearchResult` verified field-for-field; `SearchPayload` ↔ `to_core_query` ↔ core `SearchQuery` verified; `keyword_query` helper updated for new fields.
