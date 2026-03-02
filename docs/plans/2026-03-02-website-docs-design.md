# Teidelum Website & Documentation Design

## Overview

Professional website and comprehensive documentation for Teidelum, served at `lum.teidedb.com` via GitHub Pages.

## Tech Stack

- **Framework**: Astro Starlight — custom landing page + built-in docs engine
- **Deployment**: GitHub Actions → GitHub Pages at custom domain `lum.teidedb.com`
- **Location**: `website/` directory at repo root

## Logo

Flat geometric **volcanic lens/prism** — Mount Teide's peak abstracted into a clean triangular silhouette with a horizontal aperture forming a lens. Data rays enter one side, emerge unified on the other.

- **Style**: Flat, no gradients, no shadows. Works at 16px (favicon) through 200px (header).
- **Colors**: Volcanic amber `#E67E22` (prism), darker `#D35400` (aperture), lighter `#F0B27A` (data rays).
- **Wordmark**: "Teidelum" in clean sans-serif (Inter/system), weight 600, placed right of icon.
- **Variants**: Full (icon + wordmark), icon-only (favicon, small contexts), single-color monochrome.

## Landing Page

Custom Astro page at `/index.astro`, outside Starlight's doc layout. Light background (`#FAFAFA`), generous whitespace, Stripe-docs-inspired cleanliness.

### Sections

1. **Nav bar** — Logo + wordmark left. Links right: "Docs", "GitHub", "Getting Started". Sticky, white background, subtle bottom border.

2. **Hero** — Centered. Large logo. Headline: "Your data, one interface." Subhead: "A local-first MCP server that syncs work tools and connects live data sources into a single searchable, queryable index." Two CTAs: "Get Started" (amber) and "View on GitHub" (outlined).

3. **Feature cards** — 2x3 grid, capability-focused (no vendor names):
   - Full-text search (BM25 + fuzzy matching)
   - SQL analytics (columnar queries)
   - Graph traversal (relationship navigation)
   - Work tool sync (incremental data pull)
   - Live data connectors (external query adapters)
   - Single binary, zero config (data never leaves machine)

4. **Architecture diagram** — Clean SVG showing data flow. Generic source labels: "Chat & messaging", "Project & knowledge tools", "Databases & data stores", "APIs & live services" → Teidelum (search, SQL, graph, catalog) → MCP Tools → AI Agents.

5. **Quick start** — Dark code block with install/run/connect commands. Syntax-highlighted, copy button.

6. **Footer** — Minimal. "Teidelum" + GitHub link + license.

## Documentation Structure

Starlight content collections under `src/content/docs/`. Sidebar navigation:

### 1. Getting Started
- **Installation** — build from source, prerequisites (Rust toolchain)
- **Quick Start** — first run, connecting an AI agent, first query
- **Configuration** — `TEIDELUM_DATA` env var, data directory layout

### 2. Guides
- **Full-Text Search** — indexing documents, query syntax, filtering by source/date
- **SQL Queries** — table creation, supported types, query examples
- **Graph Traversal** — registering relationships, neighbors, shortest path, direction/type filtering
- **Syncing Data** — implementing `SyncSource` trait, incremental cursors, dual storage pattern
- **Building Connectors** — implementing `Connector` trait, registering with catalog

### 3. Architecture
- **Overview** — module map, design philosophy, data flow
- **Catalog System** — metadata registry, schema tracking, FK relationships
- **Query Router** — local vs remote dispatch, table resolution
- **Search Engine** — tantivy schema, indexing pipeline, BM25 ranking

### 4. Reference
- **MCP Tools** — all 5 tools with parameters, return formats, examples
- **API Reference** — `TeidelumApi` public methods, types, error handling
- **Configuration Reference** — all env vars, defaults, data directory structure

### 5. Examples
- **Agent Workflows** — end-to-end scenarios: search → SQL → graph chains
- **Custom Sync Source** — walk through building a sync adapter
- **Custom Connector** — walk through building a live query connector
- **Data Modeling** — designing tables and relationships for graph queries

## Repo Layout

```
website/
├── astro.config.mjs
├── package.json
├── tsconfig.json
├── public/
│   ├── logo.svg
│   ├── favicon.svg
│   └── og-image.png
├── src/
│   ├── assets/styles/custom.css
│   ├── components/
│   │   ├── Hero.astro
│   │   ├── Features.astro
│   │   ├── Architecture.astro
│   │   └── QuickStart.astro
│   ├── content/docs/
│   │   ├── getting-started/
│   │   ├── guides/
│   │   ├── architecture/
│   │   ├── reference/
│   │   └── examples/
│   └── pages/
│       └── index.astro
.github/workflows/
└── deploy-website.yml
```

## GitHub Actions Deployment

**Trigger**: Push to `master` affecting `website/**` or the workflow file. Manual `workflow_dispatch`.

**Steps**:
1. Checkout repo
2. Setup Node.js v20 with npm cache
3. `npm ci` in `website/`
4. `npm run build` (outputs to `website/dist/`)
5. Deploy via `actions/upload-pages-artifact` + `actions/deploy-pages`

**Config**:
- `site: 'https://lum.teidedb.com'` in `astro.config.mjs`
- No `base` path needed (custom domain root)
- CNAME file with `lum.teidedb.com` in build output
- Workflow permissions: `pages: write`, `id-token: write`

**DNS**: CNAME record `lum.teidedb.com` → `<user>.github.io`

## Visual Design

- **Theme**: Light + clean, `#FAFAFA` background
- **Accent color**: Volcanic amber `#E67E22`
- **Typography**: System sans-serif stack, clean and readable
- **No vendor names** on landing page — capability-focused only
- **Architecture diagram** uses generic source categories
