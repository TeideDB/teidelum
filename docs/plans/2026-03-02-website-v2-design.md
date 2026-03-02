# Teidelum Corporate Website — Design Document (v2)

**Date**: 2026-03-02
**Domain**: lum.teidedb.com
**Status**: Approved

## Overview

Corporate website for Teidelum, a local-first MCP server that syncs work tools (Notion, Zulip) and connects live data sources (kdb+) into a single searchable and queryable index. The site serves both AI/LLM developers building agents and technical teams evaluating data integration solutions.

## Tech Stack

Pure HTML/CSS/JS. No framework, no build tools. Deployed via GitHub Pages. Matches the org site (teidedb.com) approach exactly.

## File Structure

```
website/
├── index.html              # Landing page
├── style.css               # Shared design tokens + landing page styles
├── script.js               # Landing page interactions (nav, animations)
├── docs/
│   ├── index.html          # Docs hub / table of contents
│   ├── quick-start.html    # Getting started guide
│   ├── mcp-tools.html      # MCP tools reference (all 5 tools)
│   ├── architecture.html   # Architecture overview
│   ├── examples.html       # Agent workflow examples
│   ├── docs.css            # Docs-specific layout (sidebar + content)
│   └── docs.js             # Docs interactions (sidebar toggle, code copy)
├── assets/
│   ├── teidelum-logo.svg   # Full logo (icon + wordmark)
│   ├── teidelum-icon.svg   # Icon-only variant
│   └── favicon.svg         # Browser favicon
├── CNAME                   # lum.teidedb.com
├── .nojekyll               # Disable Jekyll processing
└── .github/workflows/
    └── deploy.yml          # GitHub Pages deployment
```

## Design System

Inherited from org site (teidedb.com):

### Colors

| Token | Value | Purpose |
|-------|-------|---------|
| `--primary` | #4b6777 | Main brand color (steel blue-gray) |
| `--primary-light` | #6b8a9e | Lighter variant |
| `--primary-pale` | #dce8ee | Badge/tag backgrounds |
| `--primary-bg` | #edf3f6 | Section tint |
| `--cream` | #f7f5f2 | Alternating section backgrounds |
| `--navy` | #0e1b24 | Code block backgrounds |
| `--text` | #1c2d38 | Primary text |
| `--gray-text` | #6b7f8e | Secondary text |
| `--border` | #e2e8ed | Borders |
| `--surface` | #ffffff | Card backgrounds |

### Typography

| Font | Purpose | Weights |
|------|---------|---------|
| Inter | Body text, UI | 400, 500, 600, 700 |
| Oswald | Headlines, section labels | 400, 500, 600, 700 |
| JetBrains Mono | Code blocks | 400, 500, 600 |

### Components (reused from org site)

- Floating pill nav with frosted glass (`backdrop-filter: blur(20px)`)
- Feature cards (icon + title + description)
- Code blocks with mac-style header (colored dots + filename + copy button)
- Badge/chip system for feature tags
- Fade-in-up scroll animations via Intersection Observer
- Responsive breakpoints: 1024px, 768px, 480px

## Landing Page

Single scrollable page with floating nav. Sections:

### Navigation
- Teidelum logo + wordmark (left)
- Links: Features | Docs | Architecture | GitHub (center/right)
- Mobile hamburger toggle

### Hero
- H1: "Your tools. One index. Local-first."
- Subtitle: Single binary MCP server that syncs work tools and connects live data sources into a searchable, queryable index.
- Feature chips: MCP Native | Full-Text Search | SQL Analytics | Graph Traversal | Zero Config
- CTA: [Get Started] [GitHub →]

### Features (#features)
- Label: "CAPABILITIES"
- 6 cards (3x2 grid → responsive):
  1. Full-Text Search — BM25 ranking with fuzzy matching, highlighted snippets
  2. SQL Analytics — Columnar queries over synced structured data
  3. Graph Traversal — Navigate FK relationships via BFS, path-finding
  4. Work Tool Sync — Incremental Notion & Zulip sync with cursor tracking
  5. Live Connectors — Query external sources (kdb+) in real-time
  6. Single Binary — No config, data never leaves your machine

### Architecture (#architecture)
- Label: "ARCHITECTURE"
- Data flow diagram (SVG or styled HTML):
  Sources (Chat, Projects, Databases) → Teidelum [search | sql | graph] → MCP Tools → AI Agents
- Brief explanation of dual storage (tantivy full-text + libteide columnar)

### Code Example (#code)
- Label: "SEE IT IN ACTION"
- Code block showing MCP tool usage / agent conversation example
- Mac-style header with dots + filename

### Footer
- Links: GitHub | TeideDB.com | Docs | MIT License
- Copyright

## Docs Section

Separate `/docs/` directory with reading-optimized two-column layout.

### Layout

```
┌──────────────────────────────────────────────┐
│  ← Back to lum.teidedb.com    [GitHub]       │
├──────────┬───────────────────────────────────┤
│ Sidebar  │  Content Area                     │
│          │  # Page Title                     │
│ Getting  │  Introduction text...             │
│ Started  │  ## Section                       │
│ > Quick  │  Content with code blocks,        │
│   Start  │  parameter tables, examples.      │
│          │                                   │
│ Reference│  Next: [MCP Tools →]              │
│   MCP    │                                   │
│   Tools  │                                   │
│          │                                   │
│ Concepts │                                   │
│   Arch.  │                                   │
│          │                                   │
│ Examples │                                   │
│   Agent  │                                   │
│   Flows  │                                   │
└──────────┴───────────────────────────────────┘
```

- Fixed sidebar on desktop, hamburger on mobile
- Active page indicator in sidebar
- Prev/next navigation at bottom of each page

### Pages (4 at launch)

1. **Quick Start** — Prerequisites (Rust toolchain), build from source, run server, connect from agent, first query
2. **MCP Tools Reference** — All 5 tools (search, sql, describe, graph, sync) with parameter tables, return formats, JSON examples
3. **Architecture** — Module overview, data flow, key design patterns (query router, catalog-driven, dual storage, incremental sync)
4. **Examples** — End-to-end agent workflows showing tool combinations (search → SQL → graph chains)

## Logo

New SVG logo for Teidelum:
- Geometric mountain/lens abstraction — shares DNA with TeideDB mountain icon, adds lens/aperture element
- Color: #4b6777 (same as org site, unified brand family)
- Variants: full (icon + "Teidelum" in Oswald Bold), icon-only, favicon
- Monochromatic, single color

## Deployment

- GitHub Pages from `website/` directory
- GitHub Actions workflow on push to master
- CNAME: lum.teidedb.com
- `.nojekyll` to disable Jekyll processing
