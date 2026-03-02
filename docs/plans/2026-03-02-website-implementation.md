# Website & Documentation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a professional website and comprehensive documentation for Teidelum at `lum.teidedb.com`, deployed via GitHub Pages.

**Architecture:** Astro Starlight project in `website/` at repo root. Custom landing page at `/`, Starlight-managed docs under `/docs/`. GitHub Actions builds and deploys to GitHub Pages on push to `master`.

**Tech Stack:** Astro 5, @astrojs/starlight, GitHub Actions, GitHub Pages, custom domain `lum.teidedb.com`.

---

### Task 1: Scaffold Astro Starlight Project

**Files:**
- Create: `website/package.json`
- Create: `website/astro.config.mjs`
- Create: `website/tsconfig.json`
- Create: `website/public/CNAME`

**Step 1: Initialize the Astro Starlight project**

```bash
cd /Users/antonkundenko/data/work/teidedb/teidelum
mkdir -p website/public website/src/pages website/src/content/docs website/src/assets/styles website/src/components
```

**Step 2: Create `website/package.json`**

```json
{
  "name": "teidelum-website",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "astro dev",
    "build": "astro build",
    "preview": "astro preview"
  },
  "dependencies": {
    "astro": "^5.0.0",
    "@astrojs/starlight": "^0.32.0",
    "sharp": "^0.33.0"
  }
}
```

**Step 3: Create `website/astro.config.mjs`**

```js
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://lum.teidedb.com',
  integrations: [
    starlight({
      title: 'Teidelum',
      logo: {
        src: './src/assets/logo.svg',
        replacesTitle: false,
      },
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/TeideDB/teidelum' },
      ],
      customCss: ['./src/assets/styles/custom.css'],
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Installation', slug: 'getting-started/installation' },
            { label: 'Quick Start', slug: 'getting-started/quickstart' },
            { label: 'Configuration', slug: 'getting-started/configuration' },
          ],
        },
        {
          label: 'Guides',
          items: [
            { label: 'Full-Text Search', slug: 'guides/search' },
            { label: 'SQL Queries', slug: 'guides/sql-queries' },
            { label: 'Graph Traversal', slug: 'guides/graph-traversal' },
            { label: 'Syncing Data', slug: 'guides/syncing-data' },
            { label: 'Building Connectors', slug: 'guides/building-connectors' },
          ],
        },
        {
          label: 'Architecture',
          items: [
            { label: 'Overview', slug: 'architecture/overview' },
            { label: 'Catalog System', slug: 'architecture/catalog-system' },
            { label: 'Query Router', slug: 'architecture/query-router' },
            { label: 'Search Engine', slug: 'architecture/search-engine' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'MCP Tools', slug: 'reference/mcp-tools' },
            { label: 'API Reference', slug: 'reference/api' },
            { label: 'Configuration Reference', slug: 'reference/configuration' },
          ],
        },
        {
          label: 'Examples',
          items: [
            { label: 'Agent Workflows', slug: 'examples/agent-workflows' },
            { label: 'Custom Sync Source', slug: 'examples/custom-sync-source' },
            { label: 'Custom Connector', slug: 'examples/custom-connector' },
            { label: 'Data Modeling', slug: 'examples/data-modeling' },
          ],
        },
      ],
    }),
  ],
});
```

**Step 4: Create `website/tsconfig.json`**

```json
{
  "extends": "astro/tsconfigs/strict"
}
```

**Step 5: Create `website/public/CNAME`**

```
lum.teidedb.com
```

**Step 6: Install dependencies and verify build**

```bash
cd website && npm install
```

Run: `npm run build`
Expected: Build succeeds (may warn about missing content, that's fine)

**Step 7: Commit**

```bash
git add website/
git commit -m "feat(website): scaffold Astro Starlight project"
```

---

### Task 2: Create SVG Logo and Favicon

**Files:**
- Create: `website/src/assets/logo.svg` (full logo for header)
- Create: `website/public/logo.svg` (standalone icon for landing page)
- Create: `website/public/favicon.svg`

The logo is a flat geometric **volcanic lens/prism**: Mount Teide's peak abstracted into a clean triangular silhouette with a horizontal aperture forming a lens. Three data rays converge from the left into the lens, and a single unified beam emerges from the right.

Colors: `#E67E22` (volcanic amber), `#D35400` (dark amber for unified beam), `#F0B27A` (light amber for input rays).

**Step 1: Create the icon SVG at `website/public/logo.svg`**

This is the standalone icon used on the landing page (no wordmark).

```svg
<svg viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg">
  <!-- Upper peak (above lens aperture) -->
  <path d="M32 6L50 30H14Z" fill="#E67E22"/>
  <!-- Lower base (below lens aperture) -->
  <path d="M14 34L50 34L60 58H4Z" fill="#E67E22"/>
  <!-- Three converging input rays (left) -->
  <line x1="1" y1="24" x2="14" y2="32" stroke="#F0B27A" stroke-width="2" stroke-linecap="round"/>
  <line x1="1" y1="32" x2="14" y2="32" stroke="#F0B27A" stroke-width="2" stroke-linecap="round"/>
  <line x1="1" y1="40" x2="14" y2="32" stroke="#F0B27A" stroke-width="2" stroke-linecap="round"/>
  <!-- Single unified beam (right) -->
  <line x1="50" y1="32" x2="63" y2="32" stroke="#D35400" stroke-width="3" stroke-linecap="round"/>
</svg>
```

**Step 2: Create the header logo at `website/src/assets/logo.svg`**

Same icon, optimized for Starlight header (smaller viewBox, no rays — just the clean prism shape for small sizes).

```svg
<svg viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
  <path d="M16 3L26 15H6Z" fill="#E67E22"/>
  <path d="M6 17L26 17L30 29H2Z" fill="#E67E22"/>
</svg>
```

**Step 3: Create the favicon at `website/public/favicon.svg`**

Same as header logo — simple prism, works at 16px.

```svg
<svg viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
  <path d="M16 3L26 15H6Z" fill="#E67E22"/>
  <path d="M6 17L26 17L30 29H2Z" fill="#E67E22"/>
</svg>
```

**Step 4: Verify assets render**

Run: `cd website && npm run dev`
Expected: Starlight header shows the logo icon. Visit `http://localhost:4321` and check the favicon.

**Step 5: Commit**

```bash
git add website/src/assets/logo.svg website/public/logo.svg website/public/favicon.svg
git commit -m "feat(website): add volcanic lens SVG logo and favicon"
```

---

### Task 3: Custom CSS Theme

**Files:**
- Create: `website/src/assets/styles/custom.css`

**Step 1: Create `website/src/assets/styles/custom.css`**

```css
/* Teidelum custom theme — volcanic amber on light */
:root {
  --sl-color-accent-low: #fef3e2;
  --sl-color-accent: #e67e22;
  --sl-color-accent-high: #d35400;
  --sl-color-white: #1a1a1a;
  --sl-color-gray-1: #444444;
  --sl-color-gray-2: #666666;
  --sl-color-gray-3: #999999;
  --sl-color-gray-4: #cccccc;
  --sl-color-gray-5: #e5e5e5;
  --sl-color-gray-6: #f5f5f5;
  --sl-color-black: #fafafa;
  --sl-font: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen,
    Ubuntu, Cantarell, sans-serif;
  --sl-text-h1: 2rem;
}

/* Landing page styles */
.landing-page {
  font-family: var(--sl-font);
  color: #1a1a1a;
  background: #fafafa;
}

.landing-nav {
  position: sticky;
  top: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1rem 2rem;
  background: #fff;
  border-bottom: 1px solid #e5e5e5;
}

.landing-nav__brand {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  text-decoration: none;
  color: #1a1a1a;
}

.landing-nav__brand img {
  height: 32px;
  width: 32px;
}

.landing-nav__brand span {
  font-size: 1.25rem;
  font-weight: 600;
}

.landing-nav__links {
  display: flex;
  gap: 1.5rem;
  list-style: none;
  margin: 0;
  padding: 0;
}

.landing-nav__links a {
  text-decoration: none;
  color: #666;
  font-size: 0.95rem;
  font-weight: 500;
  transition: color 0.15s;
}

.landing-nav__links a:hover {
  color: #e67e22;
}

/* Hero */
.hero {
  text-align: center;
  padding: 6rem 2rem 4rem;
  max-width: 720px;
  margin: 0 auto;
}

.hero__logo {
  width: 96px;
  height: 96px;
  margin-bottom: 2rem;
}

.hero__title {
  font-size: 3rem;
  font-weight: 700;
  margin: 0 0 1rem;
  letter-spacing: -0.02em;
  color: #1a1a1a;
}

.hero__subtitle {
  font-size: 1.25rem;
  color: #666;
  line-height: 1.6;
  margin: 0 0 2.5rem;
}

.hero__ctas {
  display: flex;
  gap: 1rem;
  justify-content: center;
  flex-wrap: wrap;
}

.btn {
  display: inline-flex;
  align-items: center;
  padding: 0.75rem 1.75rem;
  border-radius: 8px;
  font-size: 1rem;
  font-weight: 600;
  text-decoration: none;
  transition: all 0.15s;
  cursor: pointer;
}

.btn--primary {
  background: #e67e22;
  color: #fff;
  border: 2px solid #e67e22;
}

.btn--primary:hover {
  background: #d35400;
  border-color: #d35400;
}

.btn--outline {
  background: transparent;
  color: #1a1a1a;
  border: 2px solid #ccc;
}

.btn--outline:hover {
  border-color: #e67e22;
  color: #e67e22;
}

/* Features grid */
.features {
  max-width: 960px;
  margin: 0 auto;
  padding: 4rem 2rem;
}

.features__grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 2rem;
}

@media (max-width: 768px) {
  .features__grid {
    grid-template-columns: 1fr;
  }
}

.feature-card {
  padding: 1.5rem;
  border: 1px solid #e5e5e5;
  border-radius: 12px;
  background: #fff;
  transition: border-color 0.15s;
}

.feature-card:hover {
  border-color: #e67e22;
}

.feature-card__icon {
  font-size: 1.5rem;
  margin-bottom: 0.75rem;
  display: block;
  color: #e67e22;
}

.feature-card__title {
  font-size: 1.1rem;
  font-weight: 600;
  margin: 0 0 0.5rem;
}

.feature-card__desc {
  font-size: 0.95rem;
  color: #666;
  line-height: 1.5;
  margin: 0;
}

/* Architecture diagram */
.architecture {
  max-width: 960px;
  margin: 0 auto;
  padding: 4rem 2rem;
  text-align: center;
}

.architecture__title {
  font-size: 1.75rem;
  font-weight: 700;
  margin: 0 0 0.5rem;
}

.architecture__subtitle {
  color: #666;
  margin: 0 0 2.5rem;
  font-size: 1.05rem;
}

.architecture__diagram {
  width: 100%;
  max-width: 800px;
  margin: 0 auto;
}

/* Quick start */
.quickstart {
  max-width: 680px;
  margin: 0 auto;
  padding: 4rem 2rem;
}

.quickstart__title {
  font-size: 1.75rem;
  font-weight: 700;
  margin: 0 0 1.5rem;
  text-align: center;
}

.quickstart__code {
  background: #1e1e1e;
  border-radius: 12px;
  padding: 1.5rem 2rem;
  overflow-x: auto;
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  font-size: 0.9rem;
  line-height: 1.7;
  color: #d4d4d4;
}

.quickstart__code .comment {
  color: #6a9955;
}

.quickstart__code .command {
  color: #dcdcaa;
}

/* Footer */
.landing-footer {
  text-align: center;
  padding: 2rem;
  border-top: 1px solid #e5e5e5;
  color: #999;
  font-size: 0.9rem;
}

.landing-footer a {
  color: #666;
  text-decoration: none;
}

.landing-footer a:hover {
  color: #e67e22;
}
```

**Step 2: Verify theme applies**

Run: `cd website && npm run dev`
Expected: Starlight docs pages use volcanic amber accent color.

**Step 3: Commit**

```bash
git add website/src/assets/styles/custom.css
git commit -m "feat(website): add volcanic amber custom theme"
```

---

### Task 4: Landing Page Components

**Files:**
- Create: `website/src/components/Hero.astro`
- Create: `website/src/components/Features.astro`
- Create: `website/src/components/Architecture.astro`
- Create: `website/src/components/QuickStart.astro`

**Step 1: Create `website/src/components/Hero.astro`**

```astro
---
// Hero section — logo, headline, subtitle, CTAs
---
<section class="hero">
  <img src="/logo.svg" alt="Teidelum" class="hero__logo" />
  <h1 class="hero__title">Your data, one interface.</h1>
  <p class="hero__subtitle">
    A local-first MCP server that syncs work tools and connects live data sources
    into a single searchable, queryable index. Single binary, zero config, data never leaves your machine.
  </p>
  <div class="hero__ctas">
    <a href="/getting-started/installation/" class="btn btn--primary">Get Started</a>
    <a href="https://github.com/TeideDB/teidelum" class="btn btn--outline">View on GitHub</a>
  </div>
</section>
```

**Step 2: Create `website/src/components/Features.astro`**

```astro
---
// Feature cards grid — capability-focused, no vendor names
const features = [
  {
    icon: "&#x1F50D;",
    title: "Full-Text Search",
    desc: "BM25 ranking with fuzzy matching. Search across all connected sources with relevance-scored results and highlighted snippets.",
  },
  {
    icon: "&#x1F4CA;",
    title: "SQL Analytics",
    desc: "Run analytical queries over columnar data. Filter, aggregate, and join across structured records from any source.",
  },
  {
    icon: "&#x1F310;",
    title: "Graph Traversal",
    desc: "Navigate relationships between entities. Discover neighbors, find shortest paths, and explore connections across tables.",
  },
  {
    icon: "&#x1F504;",
    title: "Work Tool Sync",
    desc: "Incremental sync from chat platforms, project tools, and knowledge bases. Only pulls changed data on subsequent runs.",
  },
  {
    icon: "&#x26A1;",
    title: "Live Data Connectors",
    desc: "Query external databases and services in real time. Connectors translate SQL to native query languages on the fly.",
  },
  {
    icon: "&#x1F4E6;",
    title: "Single Binary, Zero Config",
    desc: "One binary, no external services. Data stays on your machine. Set TEIDELUM_DATA and run — that's it.",
  },
];
---
<section class="features">
  <div class="features__grid">
    {features.map((f) => (
      <div class="feature-card">
        <span class="feature-card__icon" set:html={f.icon} />
        <h3 class="feature-card__title">{f.title}</h3>
        <p class="feature-card__desc">{f.desc}</p>
      </div>
    ))}
  </div>
</section>
```

**Step 3: Create the architecture diagram SVG at `website/public/architecture.svg`**

```svg
<svg viewBox="0 0 900 340" fill="none" xmlns="http://www.w3.org/2000/svg" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif">
  <!-- Data Sources (left) -->
  <rect x="20" y="20" width="170" height="48" rx="8" fill="#fff" stroke="#ccc" stroke-width="1.5"/>
  <text x="105" y="49" text-anchor="middle" font-size="13" font-weight="500" fill="#444">Chat &amp; messaging</text>

  <rect x="20" y="88" width="170" height="48" rx="8" fill="#fff" stroke="#ccc" stroke-width="1.5"/>
  <text x="105" y="117" text-anchor="middle" font-size="13" font-weight="500" fill="#444">Project &amp; knowledge tools</text>

  <rect x="20" y="156" width="170" height="48" rx="8" fill="#fff" stroke="#ccc" stroke-width="1.5"/>
  <text x="105" y="185" text-anchor="middle" font-size="13" font-weight="500" fill="#444">Databases &amp; data stores</text>

  <rect x="20" y="224" width="170" height="48" rx="8" fill="#fff" stroke="#ccc" stroke-width="1.5"/>
  <text x="105" y="253" text-anchor="middle" font-size="13" font-weight="500" fill="#444">APIs &amp; live services</text>

  <!-- Arrows: sources → Teidelum -->
  <path d="M190 44 L290 100" stroke="#F0B27A" stroke-width="2" marker-end="url(#arrow-light)"/>
  <path d="M190 112 L290 130" stroke="#F0B27A" stroke-width="2" marker-end="url(#arrow-light)"/>
  <path d="M190 180 L290 160" stroke="#F0B27A" stroke-width="2" marker-end="url(#arrow-light)"/>
  <path d="M190 248 L290 190" stroke="#F0B27A" stroke-width="2" marker-end="url(#arrow-light)"/>

  <!-- Teidelum core (center) -->
  <rect x="290" y="30" width="300" height="230" rx="12" fill="#FEF3E2" stroke="#E67E22" stroke-width="2"/>
  <text x="440" y="62" text-anchor="middle" font-size="16" font-weight="700" fill="#D35400">Teidelum</text>

  <!-- Inner modules -->
  <rect x="310" y="78" width="120" height="38" rx="6" fill="#fff" stroke="#E67E22" stroke-width="1"/>
  <text x="370" y="102" text-anchor="middle" font-size="12" font-weight="500" fill="#444">Search</text>

  <rect x="450" y="78" width="120" height="38" rx="6" fill="#fff" stroke="#E67E22" stroke-width="1"/>
  <text x="510" y="102" text-anchor="middle" font-size="12" font-weight="500" fill="#444">SQL Engine</text>

  <rect x="310" y="130" width="120" height="38" rx="6" fill="#fff" stroke="#E67E22" stroke-width="1"/>
  <text x="370" y="154" text-anchor="middle" font-size="12" font-weight="500" fill="#444">Graph</text>

  <rect x="450" y="130" width="120" height="38" rx="6" fill="#fff" stroke="#E67E22" stroke-width="1"/>
  <text x="510" y="154" text-anchor="middle" font-size="12" font-weight="500" fill="#444">Catalog</text>

  <rect x="310" y="190" width="260" height="38" rx="6" fill="#fff" stroke="#E67E22" stroke-width="1"/>
  <text x="440" y="214" text-anchor="middle" font-size="12" font-weight="500" fill="#444">Unified API</text>

  <!-- Arrow: Teidelum → MCP -->
  <path d="M590 145 L650 145" stroke="#D35400" stroke-width="2.5" marker-end="url(#arrow-dark)"/>

  <!-- MCP Tools -->
  <rect x="650" y="100" width="120" height="90" rx="10" fill="#fff" stroke="#D35400" stroke-width="1.5"/>
  <text x="710" y="135" text-anchor="middle" font-size="14" font-weight="600" fill="#D35400">MCP</text>
  <text x="710" y="155" text-anchor="middle" font-size="14" font-weight="600" fill="#D35400">Tools</text>

  <!-- Arrow: MCP → AI Agents -->
  <path d="M770 145 L810 145" stroke="#D35400" stroke-width="2.5" marker-end="url(#arrow-dark)"/>

  <!-- AI Agents -->
  <rect x="810" y="100" width="80" height="90" rx="10" fill="#D35400" stroke="none"/>
  <text x="850" y="140" text-anchor="middle" font-size="13" font-weight="600" fill="#fff">AI</text>
  <text x="850" y="158" text-anchor="middle" font-size="13" font-weight="600" fill="#fff">Agents</text>

  <!-- Arrow markers -->
  <defs>
    <marker id="arrow-light" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
      <path d="M0 0 L8 3 L0 6" fill="none" stroke="#F0B27A" stroke-width="1.5"/>
    </marker>
    <marker id="arrow-dark" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
      <path d="M0 0 L8 3 L0 6" fill="none" stroke="#D35400" stroke-width="1.5"/>
    </marker>
  </defs>
</svg>
```

**Step 4: Create `website/src/components/Architecture.astro`**

```astro
---
// Architecture diagram section
---
<section class="architecture">
  <h2 class="architecture__title">How it works</h2>
  <p class="architecture__subtitle">
    Data flows from your tools through a unified engine to AI agents via MCP.
  </p>
  <img src="/architecture.svg" alt="Teidelum architecture diagram" class="architecture__diagram" />
</section>
```

**Step 5: Create `website/src/components/QuickStart.astro`**

```astro
---
// Quick start code block
---
<section class="quickstart">
  <h2 class="quickstart__title">Get started in seconds</h2>
  <div class="quickstart__code">
    <div><span class="comment"># Clone and build</span></div>
    <div><span class="command">git clone https://github.com/TeideDB/teidelum.git</span></div>
    <div><span class="command">cd teidelum && cargo build --release</span></div>
    <div>&nbsp;</div>
    <div><span class="comment"># Run the MCP server</span></div>
    <div><span class="command">./target/release/teidelum</span></div>
    <div>&nbsp;</div>
    <div><span class="comment"># Add to your MCP client config</span></div>
    <div><span class="command">&#123; "mcpServers": &#123; "teidelum": &#123; "command": "./target/release/teidelum" &#125; &#125; &#125;</span></div>
  </div>
</section>
```

**Step 6: Commit**

```bash
git add website/src/components/ website/public/architecture.svg
git commit -m "feat(website): add landing page components"
```

---

### Task 5: Assemble Landing Page

**Files:**
- Create: `website/src/pages/index.astro`

**Step 1: Create `website/src/pages/index.astro`**

```astro
---
import Hero from '../components/Hero.astro';
import Features from '../components/Features.astro';
import Architecture from '../components/Architecture.astro';
import QuickStart from '../components/QuickStart.astro';
import '../assets/styles/custom.css';
---
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Teidelum — Your data, one interface</title>
  <meta name="description" content="A local-first MCP server that syncs work tools and connects live data sources into a single searchable, queryable index." />
  <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
</head>
<body class="landing-page">
  <nav class="landing-nav">
    <a href="/" class="landing-nav__brand">
      <img src="/logo.svg" alt="" width="32" height="32" />
      <span>Teidelum</span>
    </a>
    <ul class="landing-nav__links">
      <li><a href="/getting-started/installation/">Docs</a></li>
      <li><a href="/getting-started/quickstart/">Getting Started</a></li>
      <li><a href="https://github.com/TeideDB/teidelum">GitHub</a></li>
    </ul>
  </nav>

  <main>
    <Hero />
    <Features />
    <Architecture />
    <QuickStart />
  </main>

  <footer class="landing-footer">
    Teidelum &middot;
    <a href="https://github.com/TeideDB/teidelum">GitHub</a> &middot;
    MIT License
  </footer>
</body>
</html>
```

**Step 2: Verify landing page renders**

Run: `cd website && npm run dev`
Expected: Visit `http://localhost:4321/` — landing page with nav, hero, features grid, architecture diagram, quick start block, and footer. All using volcanic amber colors on light background.

**Step 3: Commit**

```bash
git add website/src/pages/index.astro
git commit -m "feat(website): assemble landing page"
```

---

### Task 6: Getting Started Documentation

**Files:**
- Create: `website/src/content/docs/getting-started/installation.md`
- Create: `website/src/content/docs/getting-started/quickstart.md`
- Create: `website/src/content/docs/getting-started/configuration.md`

**Step 1: Create `website/src/content/docs/getting-started/installation.md`**

```markdown
---
title: Installation
description: Build Teidelum from source
---

## Prerequisites

- **Rust toolchain** (1.75+) — install via [rustup](https://rustup.rs/):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **C compiler** — required by teide (the columnar engine). On macOS, Xcode CLT (`xcode-select --install`). On Linux, `build-essential`.
- **Git** — to clone the repository.

## Build from Source

```bash
git clone https://github.com/TeideDB/teidelum.git
cd teidelum
cargo build --release
```

The binary is at `./target/release/teidelum`.

## Verify Installation

```bash
./target/release/teidelum --help
```

Or run the test suite:

```bash
cargo test
```

## Development Build

For faster iteration during development:

```bash
cargo build    # debug build (faster compile, slower runtime)
cargo run      # build and run
cargo check    # type-check only (fastest feedback)
```
```

**Step 2: Create `website/src/content/docs/getting-started/quickstart.md`**

```markdown
---
title: Quick Start
description: Get Teidelum running and connected to an AI agent in minutes
---

## Run the Server

Teidelum serves MCP over stdio. Start it:

```bash
./target/release/teidelum
```

On first run, it generates demo data (team members, project tasks, incidents, and sample documents) so you can explore immediately.

## Connect to Your AI Agent

Add Teidelum to your MCP client configuration. For Claude Desktop or Claude Code, add to your MCP settings:

```json
{
  "mcpServers": {
    "teidelum": {
      "command": "/path/to/teidelum"
    }
  }
}
```

Replace `/path/to/teidelum` with the absolute path to your built binary.

## Try Your First Queries

Once connected, your AI agent has access to five tools:

### Search for documents

Ask your agent: *"Search for authentication docs"*

This triggers the `search` tool, which performs full-text search with BM25 ranking across all indexed documents.

### Query structured data

Ask: *"Show me all team members"*

This triggers the `sql` tool:

```sql
SELECT * FROM team_members
```

### Explore relationships

Ask: *"Find all tasks assigned to Alice Chen"*

This triggers the `graph` tool, traversing FK relationships between `project_tasks` and `team_members`.

### See what's available

Ask: *"Describe the available data"*

This triggers the `describe` tool, returning all tables, their schemas, and registered relationships.
```

**Step 3: Create `website/src/content/docs/getting-started/configuration.md`**

```markdown
---
title: Configuration
description: Configure Teidelum's data directory and logging
---

## Data Directory

Teidelum stores all data in a single directory. Set it via the `TEIDELUM_DATA` environment variable:

```bash
export TEIDELUM_DATA=/path/to/data
./target/release/teidelum
```

Default: `./data/` (relative to the working directory).

### Directory Layout

```
data/
├── tables/           # Splayed columnar tables (teide format)
│   ├── sym           # Shared symbol file
│   ├── team_members/ # One directory per table
│   │   ├── .d        # Table marker
│   │   ├── name      # Column files
│   │   └── role
│   └── project_tasks/
├── docs/             # Markdown documents (indexed for search)
│   ├── auth-rfc.md
│   └── deployment-runbook.md
└── index/            # Tantivy full-text search index
```

## Logging

Teidelum uses the `RUST_LOG` environment variable for log level control:

```bash
RUST_LOG=info ./target/release/teidelum    # default
RUST_LOG=debug ./target/release/teidelum   # verbose
RUST_LOG=warn ./target/release/teidelum    # quiet
```

Logs go to **stderr** (stdout is reserved for the MCP stdio transport).

## Demo Data

On first run, if the `tables/` or `docs/` directories don't exist, Teidelum generates demo data:

- **team_members** — names, roles, departments
- **project_tasks** — titles, statuses, priorities, assignees
- **incidents** — descriptions, severities, reporters
- **Sample documents** — markdown files covering auth, deployment, rate limiting, etc.

To start fresh, delete the data directory:

```bash
rm -rf data/
./target/release/teidelum
```
```

**Step 4: Verify docs render**

Run: `cd website && npm run dev`
Expected: Visit `http://localhost:4321/getting-started/installation/` — docs page with sidebar, content, and navigation.

**Step 5: Commit**

```bash
git add website/src/content/docs/getting-started/
git commit -m "docs(website): add getting started section"
```

---

### Task 7: Guides Documentation

**Files:**
- Create: `website/src/content/docs/guides/search.md`
- Create: `website/src/content/docs/guides/sql-queries.md`
- Create: `website/src/content/docs/guides/graph-traversal.md`
- Create: `website/src/content/docs/guides/syncing-data.md`
- Create: `website/src/content/docs/guides/building-connectors.md`

**Step 1: Create `website/src/content/docs/guides/search.md`**

```markdown
---
title: Full-Text Search
description: Index documents and run full-text search queries
---

Teidelum uses [tantivy](https://github.com/quickwit-oss/tantivy) for full-text search with BM25 ranking and fuzzy matching.

## How Documents Are Indexed

Documents are indexed with four fields:

| Field | Type | Purpose |
|-------|------|---------|
| `id` | STRING (stored) | Unique document identifier |
| `source` | STRING (stored) | Origin (e.g., "notion", "zulip") |
| `title` | TEXT (stored) | Document title — tokenized and searchable |
| `body` | TEXT (stored) | Full content — tokenized and searchable |

The `title` and `body` fields are tokenized for full-text search. The `id` and `source` fields are stored for filtering and attribution.

## Search Query Parameters

The `search` MCP tool accepts:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `query` | string | required | Full-text search query |
| `sources` | string[] | all | Filter to specific sources |
| `limit` | number | 10 | Maximum results |
| `date_from` | string | none | ISO 8601 date lower bound |
| `date_to` | string | none | ISO 8601 date upper bound |

## Query Syntax

Queries use tantivy's query parser, which supports:

- **Simple terms**: `authentication` — matches documents containing the word
- **Multi-word**: `JWT token rotation` — matches documents containing any of the words (OR by default)
- **Phrase search**: `"token rotation"` — matches the exact phrase
- **Field-scoped**: `title:authentication` — search only in the title field

## Search Results

Each result includes:

```json
{
  "id": "auth-rfc",
  "source": "notion",
  "title": "Authentication RFC",
  "snippet": "This document covers <b>JWT</b> <b>authentication</b> and <b>token</b> management",
  "score": 12.34
}
```

- **snippet**: HTML-highlighted excerpt showing matched terms in context
- **score**: BM25 relevance score (higher = more relevant)

## Filtering by Source

To search only within specific sources:

```json
{
  "query": "deployment",
  "sources": ["notion"]
}
```
```

**Step 2: Create `website/src/content/docs/guides/sql-queries.md`**

```markdown
---
title: SQL Queries
description: Run analytical queries over structured data
---

Teidelum routes SQL queries to its columnar engine (teide) for local tables. Tables are stored in a splayed columnar format optimized for analytical queries.

## Supported Data Types

| Type | SQL Type | Description |
|------|----------|-------------|
| `bool` | BOOLEAN | True/false |
| `i32` | BIGINT | 32-bit integer (stored as BIGINT) |
| `i64` | BIGINT | 64-bit integer |
| `f64` | DOUBLE | 64-bit floating point |
| `string` | VARCHAR | Variable-length text |
| `date` | DATE | Calendar date |
| `time` | TIME | Time of day |
| `timestamp` | TIMESTAMP | Date and time |

## Query Examples

### List all tables

Use the `describe` tool to see available tables and their schemas.

### Basic SELECT

```sql
SELECT name, role FROM team_members
```

### Filtering

```sql
SELECT title, status FROM project_tasks WHERE priority = 'high'
```

### Aggregation

```sql
SELECT status, count(*) as cnt
FROM project_tasks
GROUP BY status
```

### Ordering and limits

```sql
SELECT title, priority
FROM project_tasks
ORDER BY priority
LIMIT 10
```

## Query Results

Results are returned as JSON with column schemas and rows:

```json
{
  "columns": [
    { "name": "name", "dtype": "string" },
    { "name": "role", "dtype": "string" }
  ],
  "rows": [
    ["Alice Chen", "Backend Engineer"],
    ["Bob Smith", "Frontend Engineer"]
  ]
}
```

## Error Handling

Invalid SQL returns an error message. Common issues:

- **Table not found**: Check available tables with `describe`
- **Column not found**: Verify column names in the table schema
- **Syntax error**: Standard SQL syntax is expected
```

**Step 3: Create `website/src/content/docs/guides/graph-traversal.md`**

```markdown
---
title: Graph Traversal
description: Navigate relationships between entities using BFS
---

Teidelum's graph engine performs BFS traversal over foreign key relationships registered in the catalog. It supports neighbor discovery and shortest-path finding.

## Registering Relationships

Before graph traversal works, you must register FK relationships. Each relationship links a column in one table to a column in another:

```rust
Relationship {
    from_table: "project_tasks",
    from_col: "assignee",
    to_table: "team_members",
    to_col: "name",
    relation: "assigned_to",
}
```

This means: `project_tasks.assignee` references `team_members.name`, and the relationship is called `assigned_to`.

## Graph Operations

### Neighbors

Discover all entities reachable from a starting node up to a given depth.

Parameters:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `table` | string | required | Starting node's table |
| `key` | string | required | Node identifier value |
| `key_col` | string | "name" | Column used to identify the node |
| `operation` | string | "neighbors" | Set to "neighbors" |
| `depth` | number | 2 | Max traversal hops (capped at 10) |
| `direction` | string | "both" | "forward", "reverse", or "both" |
| `rel_types` | string[] | all | Filter to specific relationship types |

Example — find everything connected to Alice Chen within 2 hops:

```json
{
  "table": "team_members",
  "key": "Alice Chen",
  "key_col": "name",
  "operation": "neighbors",
  "depth": 2,
  "direction": "both"
}
```

Response:

```json
{
  "nodes": [
    { "table": "team_members", "key": "Alice Chen", "properties": { "role": "Backend Engineer" } },
    { "table": "project_tasks", "key": "Implement JWT rotation", "properties": { "status": "in_progress" } }
  ],
  "edges": [
    {
      "from_table": "project_tasks", "from_key": "Implement JWT rotation",
      "to_table": "team_members", "to_key": "Alice Chen",
      "relation": "assigned_to"
    }
  ]
}
```

### Path

Find the shortest path between two specific nodes.

Additional parameters for path operations:

| Parameter | Type | Description |
|-----------|------|-------------|
| `to_table` | string | Target node's table (required) |
| `to_key` | string | Target node's identifier (required) |
| `to_key_col` | string | Target's key column (defaults to `key_col`) |

Example — find path from a task to a team member:

```json
{
  "table": "project_tasks",
  "key": "Implement JWT rotation",
  "key_col": "title",
  "operation": "path",
  "to_table": "team_members",
  "to_key": "Alice Chen",
  "to_key_col": "name",
  "depth": 5
}
```

Response:

```json
{
  "found": true,
  "path": [
    { "table": "project_tasks", "key": "Implement JWT rotation" },
    { "table": "team_members", "key": "Alice Chen" }
  ],
  "hops": 1
}
```

## Direction Filtering

- **"forward"**: Follow relationships in the defined direction (`from_table` → `to_table`)
- **"reverse"**: Follow relationships backwards (`to_table` → `from_table`)
- **"both"**: Follow relationships in either direction

## Depth Limits

Maximum traversal depth is capped at **10 hops** to prevent unbounded query storms. The `depth` parameter controls how many hops to traverse (default: 2).
```

**Step 4: Create `website/src/content/docs/guides/syncing-data.md`**

```markdown
---
title: Syncing Data
description: Pull data from external tools into Teidelum
---

Sync sources pull data from external APIs and split it into two streams:

1. **Structured records** → columnar tables (for SQL queries)
2. **Search documents** → full-text index (for search)

This dual-storage pattern ensures data is queryable both structurally and by content.

## The SyncSource Trait

Every sync source implements the `SyncSource` trait:

```rust
#[async_trait]
pub trait SyncSource: Send + Sync {
    /// Unique name for this source (e.g. "notion", "zulip").
    fn name(&self) -> &str;

    /// Run an incremental sync. The cursor is opaque state from the
    /// previous run (None on first sync).
    async fn sync(
        &self,
        cursor: Option<&str>,
    ) -> Result<(SyncOutput, Option<String>)>;
}
```

## SyncOutput

A sync run produces `SyncOutput` containing two collections:

```rust
pub struct SyncOutput {
    pub records: Vec<StructuredRecord>,
    pub documents: Vec<SearchDocument>,
}
```

### StructuredRecord

Columnar data destined for SQL tables:

```rust
pub struct StructuredRecord {
    pub table: String,
    pub fields: serde_json::Map<String, serde_json::Value>,
}
```

### SearchDocument

Free-text content destined for the search index:

```rust
pub struct SearchDocument {
    pub id: String,
    pub source: String,
    pub title: String,
    pub body: String,
    pub metadata: serde_json::Map<String, serde_json::Value>,
}
```

## Incremental Sync

The `cursor` parameter enables incremental sync:

1. **First sync**: `cursor` is `None`. Pull all data and return a cursor string.
2. **Subsequent syncs**: `cursor` contains the previous run's state. Pull only changed data since that cursor.

The cursor format is opaque — each source defines its own (timestamps, page tokens, etc.).

## Triggering Sync

Use the `sync` MCP tool:

```json
{ "source": "my_source" }
```

Or omit `source` to sync all registered sources.
```

**Step 5: Create `website/src/content/docs/guides/building-connectors.md`**

```markdown
---
title: Building Connectors
description: Create live query adapters for external data sources
---

Connectors query external data sources in real time, without storing data locally. The query router dispatches to connectors for tables marked as `remote` in the catalog.

## The Connector Trait

```rust
#[async_trait]
pub trait Connector: Send + Sync {
    /// Unique name for this connector (e.g. "kdb").
    fn name(&self) -> &str;

    /// Return the schemas of tables available through this connector.
    async fn list_tables(&self) -> Result<Vec<(String, Vec<ColumnSchema>)>>;

    /// Execute a SQL query against the remote source.
    async fn query(&self, sql: &str) -> Result<QueryResult>;
}
```

## Implementing a Connector

A connector needs to:

1. **List available tables** with their column schemas
2. **Translate SQL** to the source's native query language
3. **Execute queries** and return results as `QueryResult`

### QueryResult Format

```rust
pub struct QueryResult {
    pub columns: Vec<ColumnSchema>,
    pub rows: Vec<Vec<Value>>,
}
```

Where `Value` is one of:

```rust
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}
```

### ColumnSchema

```rust
pub struct ColumnSchema {
    pub name: String,
    pub dtype: String,  // "bool", "i64", "f64", "string", etc.
}
```

## Registering a Connector

After implementing the trait, register the connector's tables in the catalog with `StorageType::Remote`. The query router will then dispatch matching SQL queries to your connector instead of the local engine.
```

**Step 6: Commit**

```bash
git add website/src/content/docs/guides/
git commit -m "docs(website): add guides section"
```

---

### Task 8: Architecture Documentation

**Files:**
- Create: `website/src/content/docs/architecture/overview.md`
- Create: `website/src/content/docs/architecture/catalog-system.md`
- Create: `website/src/content/docs/architecture/query-router.md`
- Create: `website/src/content/docs/architecture/search-engine.md`

**Step 1: Create `website/src/content/docs/architecture/overview.md`**

```markdown
---
title: Architecture Overview
description: How Teidelum's modules fit together
---

Teidelum is a single-crate Rust application organized into focused modules behind a unified API facade.

## Module Map

| Module | Role |
|--------|------|
| `main.rs` | Entrypoint: opens `TeidelumApi`, registers relationships, serves MCP over stdio |
| `api.rs` | Unified API: wraps catalog, search, router, graph behind thread-safe interface |
| `mcp.rs` | MCP tool definitions via `rmcp`; delegates to `TeidelumApi` |
| `router.rs` | Query router: dispatches SQL to the local columnar engine |
| `search.rs` | Tantivy wrapper: full-text search with BM25 ranking |
| `catalog.rs` | Metadata catalog: schemas, FK relationships, local vs remote tracking |
| `graph.rs` | Graph traversal: BFS over catalog FK relationships |
| `connector/` | `Connector` trait for live external queries |
| `sync/` | `SyncSource` trait for incremental data pull |
| `demo.rs` | Demo data generator for first-run experience |

## Data Flow

```
External APIs ──┐
                ├──▶ Sync Sources ──▶ Structured Records ──▶ SQL Engine (teide)
                │                  └──▶ Search Documents  ──▶ Search Index (tantivy)
                │
External DBs ───┴──▶ Connectors ──▶ Live Query Results
                                        │
                         ┌───────────────┘
                         ▼
              ┌─────────────────────┐
              │   TeidelumApi       │
              │  ┌───────┬────────┐ │
              │  │Catalog│ Search │ │
              │  ├───────┼────────┤ │
              │  │Router │ Graph  │ │
              │  └───────┴────────┘ │
              └─────────┬───────────┘
                        ▼
                 MCP Tools (stdio)
                        ▼
                    AI Agents
```

## Design Principles

- **Unified API**: All subsystems are accessed through `TeidelumApi`. MCP server, tests, and future plugins all go through this single facade.
- **Dual Storage**: Sync modules split data into structured records (columnar tables for SQL) and search documents (full-text index). This means the same data is queryable both analytically and by content.
- **Catalog-Driven**: The catalog describes all available data. The query router uses it to dispatch queries. The `describe` tool exposes it. The graph engine builds its topology from it.
- **Thread Safety**: `RwLock` for catalog and graph (concurrent reads), `Arc` for search engine and router (shared ownership), `Mutex` for the teide session (C FFI).

## Key Dependencies

| Dependency | Purpose |
|------------|---------|
| `rmcp` | MCP protocol implementation |
| `tantivy` | Full-text search engine |
| `teide` | Local columnar database engine |
| `tokio` | Async runtime |
| `serde` / `serde_json` | Serialization |
| `anyhow` / `thiserror` | Error handling |
```

**Step 2: Create `website/src/content/docs/architecture/catalog-system.md`**

```markdown
---
title: Catalog System
description: How Teidelum tracks tables, schemas, and relationships
---

The catalog is the metadata registry at the heart of Teidelum. It tracks what data is available, where it lives, and how it's related.

## TableEntry

Each registered table has a `TableEntry`:

```rust
pub struct TableEntry {
    pub name: String,           // Table name (valid SQL identifier)
    pub source: String,         // Origin (e.g., "notion", "demo")
    pub storage: StorageType,   // Local or Remote
    pub columns: Vec<ColumnInfo>,
    pub row_count: Option<u64>,
}
```

### StorageType

- **Local**: Data is stored in teide's columnar format on disk. SQL queries execute locally.
- **Remote**: Data lives in an external system. SQL queries are dispatched to connectors.

## Relationships

FK relationships link columns across tables:

```rust
pub struct Relationship {
    pub from_table: String,
    pub from_col: String,
    pub to_table: String,
    pub to_col: String,
    pub relation: String,  // Label (e.g., "assigned_to")
}
```

Relationships can be registered before the referenced tables exist. The graph engine rebuilds its topology whenever the catalog changes.

## Identifier Validation

All table names, column names, and relationship fields are validated as safe SQL identifiers: must start with a letter or underscore, followed by alphanumeric characters or underscores. This prevents SQL injection in dynamically constructed queries.

## The `describe` Tool

The catalog powers the `describe` MCP tool, which returns all tables and relationships as JSON. It supports optional source filtering:

```json
{
  "tables": [
    {
      "name": "team_members",
      "source": "demo",
      "storage": "local",
      "columns": [
        { "name": "name", "dtype": "string" },
        { "name": "role", "dtype": "string" }
      ],
      "row_count": 10
    }
  ],
  "relationships": [
    {
      "from_table": "project_tasks",
      "from_col": "assignee",
      "to_table": "team_members",
      "to_col": "name",
      "relation": "assigned_to"
    }
  ]
}
```
```

**Step 3: Create `website/src/content/docs/architecture/query-router.md`**

```markdown
---
title: Query Router
description: How SQL queries are dispatched to the right engine
---

The query router receives SQL queries and dispatches them to the appropriate engine based on the catalog's metadata.

## Local Queries

For tables with `StorageType::Local`, queries go to the teide columnar engine. Teide stores data in a splayed format — one file per column — optimized for analytical operations.

### Table Loading

On startup, Teidelum scans the `tables/` directory for splayed tables (directories containing a `.d` marker file). Each table is loaded into teide's in-memory engine:

```rust
pub fn load_splayed(
    &self,
    name: &str,
    dir: &Path,
    sym_path: Option<&Path>,
) -> Result<()>
```

The optional `sym_path` points to a shared symbol file used for enumerated string columns.

## Thread Safety

Teide's `Session` contains raw pointers from its C FFI layer, making it neither `Send` nor `Sync`. The router wraps it in a `Mutex` to ensure exclusive access:

```rust
pub struct QueryRouter {
    session: Mutex<teide::Session>,
}
```

All query execution goes through `query_sync`, which locks the mutex:

```rust
pub fn query_sync(&self, sql: &str) -> Result<QueryResult> {
    let mut session = self.session.lock().unwrap();
    let result = session.execute(sql)?;
    // ... convert to QueryResult
}
```

## Query Results

All queries return a uniform `QueryResult`:

```rust
pub struct QueryResult {
    pub columns: Vec<ColumnSchema>,
    pub rows: Vec<Vec<Value>>,
}
```

DDL statements (CREATE TABLE, DROP TABLE) return a single-row result with a status message.
```

**Step 4: Create `website/src/content/docs/architecture/search-engine.md`**

```markdown
---
title: Search Engine
description: Full-text indexing and search with tantivy
---

Teidelum uses [tantivy](https://github.com/quickwit-oss/tantivy) for full-text search. Tantivy is a Rust search engine library inspired by Apache Lucene.

## Index Schema

The search index has four fields:

| Field | Tantivy Type | Flags | Purpose |
|-------|-------------|-------|---------|
| `id` | text | STRING, STORED | Unique document ID, exact match only |
| `source` | text | STRING, STORED | Source attribution, exact match only |
| `title` | text | TEXT, STORED | Tokenized for full-text search |
| `body` | text | TEXT, STORED | Tokenized for full-text search |

`STRING` fields are indexed as-is (no tokenization). `TEXT` fields are tokenized into terms for full-text search.

## Indexing Pipeline

Documents are indexed in batches via `index_documents`:

1. Acquire an `IndexWriter` with a 50MB heap budget
2. Add each document to the writer
3. Commit the batch
4. Reload the reader so new documents are immediately searchable

## Search Pipeline

1. Parse the query using tantivy's `QueryParser` against `title` and `body` fields
2. Execute search with `TopDocs` collector (limited by `limit` parameter)
3. Generate highlighted snippets using `SnippetGenerator`
4. Filter results by source if `sources` parameter is set
5. Return results ordered by BM25 relevance score

## Storage

The search index is stored on disk using memory-mapped files (`MmapDirectory`). The index directory is at `{TEIDELUM_DATA}/index/`.

If the index directory already exists, it's opened. Otherwise, a new empty index is created.

## Reader Policy

The index reader uses `ReloadPolicy::OnCommitWithDelay` — it automatically picks up new documents after commits, with a small delay for efficiency.
```

**Step 5: Commit**

```bash
git add website/src/content/docs/architecture/
git commit -m "docs(website): add architecture section"
```

---

### Task 9: Reference Documentation

**Files:**
- Create: `website/src/content/docs/reference/mcp-tools.md`
- Create: `website/src/content/docs/reference/api.md`
- Create: `website/src/content/docs/reference/configuration.md`

**Step 1: Create `website/src/content/docs/reference/mcp-tools.md`**

```markdown
---
title: MCP Tools
description: Complete reference for all five MCP tools
---

Teidelum exposes five tools via the Model Context Protocol. AI agents call these tools to search, query, explore, and sync data.

## search

**Description:** Full-text search across all connected sources.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `query` | string | yes | — | Search query text |
| `sources` | string[] | no | all | Filter to specific sources |
| `limit` | number | no | 10 | Max results to return |
| `date_from` | string | no | — | ISO 8601 date lower bound |
| `date_to` | string | no | — | ISO 8601 date upper bound |

**Returns:** Array of search results with `id`, `source`, `title`, `snippet` (HTML), and `score`.

**Example:**

```json
{
  "query": "authentication JWT",
  "sources": ["notion"],
  "limit": 5
}
```

---

## sql

**Description:** Run analytical queries over structured data from all sources.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | yes | SQL query to execute |

**Returns:** `QueryResult` with `columns` (name + dtype) and `rows` (array of arrays).

**Example:**

```json
{
  "query": "SELECT name, role FROM team_members WHERE department = 'Engineering'"
}
```

---

## describe

**Description:** List available tables, schemas, and relationships.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | string | no | all | Filter to a specific source |

**Returns:** JSON with `tables` array and `relationships` array.

**Example:**

```json
{
  "source": "demo"
}
```

---

## graph

**Description:** Traverse relationships between entities (neighbors, paths).

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `table` | string | yes | — | Starting node's table |
| `key` | string | yes | — | Node identifier value |
| `key_col` | string | no | "name" | Key column name |
| `operation` | string | no | "neighbors" | "neighbors" or "path" |
| `depth` | number | no | 2 | Max traversal hops (max 10) |
| `direction` | string | no | "both" | "forward", "reverse", or "both" |
| `rel_types` | string[] | no | all | Filter relationship types |
| `to_table` | string | path only | — | Target table (path operation) |
| `to_key` | string | path only | — | Target key (path operation) |
| `to_key_col` | string | no | key_col | Target key column (path operation) |

**Returns (neighbors):** `{ nodes: [...], edges: [...] }`

**Returns (path):** `{ found: bool, path: [...], hops: number }`

**Example (neighbors):**

```json
{
  "table": "team_members",
  "key": "Alice Chen",
  "operation": "neighbors",
  "depth": 2,
  "direction": "both"
}
```

**Example (path):**

```json
{
  "table": "project_tasks",
  "key": "Implement JWT rotation",
  "key_col": "title",
  "operation": "path",
  "to_table": "team_members",
  "to_key": "Alice Chen",
  "depth": 5
}
```

---

## sync

**Description:** Trigger incremental sync for connected sources.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | string | no | all | Sync a specific source |

**Returns:** Sync status with counts of added/updated/deleted records.

**Example:**

```json
{
  "source": "notion"
}
```

:::note
Sync sources are not yet implemented in the current release. The tool returns a placeholder response.
:::
```

**Step 2: Create `website/src/content/docs/reference/api.md`**

```markdown
---
title: API Reference
description: TeidelumApi public methods and types
---

`TeidelumApi` is the unified programmatic interface. All subsystems are accessed through this single facade.

## Construction

### `TeidelumApi::new(data_dir: &Path) -> Result<Self>`

Create an empty instance. Initializes the search index, query router, catalog, and graph engine. No data is loaded.

### `TeidelumApi::open(data_dir: &Path) -> Result<Self>`

Open an existing data directory. Loads all splayed tables from `{data_dir}/tables/` and indexes all markdown documents from `{data_dir}/docs/`.

## Table Operations

### `create_table(name, source, columns, rows) -> Result<()>`

Create a new table with the given schema and data.

- `name: &str` — table name (must be a valid SQL identifier)
- `source: &str` — origin identifier
- `columns: &[ColumnSchema]` — column definitions
- `rows: &[Vec<Value>]` — row data

Rows are inserted in batches of 1,000. If insertion fails, the table is dropped (rollback).

### `register_table(entry: TableEntry)`

Register a pre-built table entry in the catalog (e.g., for remote connectors).

### `query(sql: &str) -> Result<QueryResult>`

Execute a SQL query against the local engine.

## Search Operations

### `add_documents(docs: &[SearchDocument]) -> Result<usize>`

Index documents into the full-text search engine. Returns the count of documents indexed.

### `search(query: &SearchQuery) -> Result<Vec<SearchResult>>`

Run a full-text search query.

## Relationship Operations

### `register_relationship(rel: Relationship) -> Result<()>`

Register a single FK relationship and rebuild the graph engine.

### `register_relationships(rels: Vec<Relationship>) -> Result<()>`

Register multiple relationships in bulk. Validates all relationships before mutating the catalog. Rebuilds the graph engine once at the end.

## Graph Operations

### `neighbors(table, key_col, key_value, depth, direction, rel_types) -> Result<JsonValue>`

Find all nodes reachable from the starting node up to the given depth.

### `path(table, key_col, key_value, to_table, to_key_col, to_key, depth, direction, rel_types) -> Result<JsonValue>`

Find the shortest path between two nodes.

## Catalog Operations

### `describe(source_filter: Option<&str>) -> Result<JsonValue>`

Return a JSON description of all tables and relationships, optionally filtered by source.

## Accessor Methods

### `search_engine() -> &Arc<SearchEngine>`

Access the search engine directly.

### `query_router() -> &Arc<QueryRouter>`

Access the query router directly.
```

**Step 3: Create `website/src/content/docs/reference/configuration.md`**

```markdown
---
title: Configuration Reference
description: All environment variables, defaults, and directory structure
---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `TEIDELUM_DATA` | `./data/` | Path to the data directory |
| `RUST_LOG` | `info` | Log level (`error`, `warn`, `info`, `debug`, `trace`) |

## Data Directory Structure

```
{TEIDELUM_DATA}/
├── tables/              # Splayed columnar tables
│   ├── sym              # Shared symbol file for enumerated strings
│   └── {table_name}/    # One directory per table
│       ├── .d           # Table marker file
│       └── {column}     # One file per column (binary format)
├── docs/                # Markdown documents for full-text indexing
│   └── *.md             # Each .md file is indexed
└── index/               # Tantivy search index
    └── (tantivy files)  # Managed by tantivy, do not modify
```

## Table Directory Format

Each table is a directory containing:

- **`.d`** — marker file indicating this is a splayed table
- **Column files** — one binary file per column, named after the column

The `sym` file in the `tables/` root is a shared symbol table for enumerated string columns (categorical data).

## Logging Output

All logs go to **stderr**. Stdout is reserved for the MCP stdio transport. This means you can pipe Teidelum's output to an MCP client while still seeing logs:

```bash
RUST_LOG=debug ./teidelum 2>teidelum.log
```

## Defaults

| Setting | Default Value |
|---------|---------------|
| Data directory | `./data/` |
| Search result limit | 10 |
| Graph traversal depth | 2 |
| Max graph depth | 10 |
| Key column for graph | `"name"` |
| Graph direction | `"both"` |
| Graph operation | `"neighbors"` |
| Index writer heap | 50 MB |
| Insert batch size | 1,000 rows |
```

**Step 4: Commit**

```bash
git add website/src/content/docs/reference/
git commit -m "docs(website): add reference section"
```

---

### Task 10: Examples Documentation

**Files:**
- Create: `website/src/content/docs/examples/agent-workflows.md`
- Create: `website/src/content/docs/examples/custom-sync-source.md`
- Create: `website/src/content/docs/examples/custom-connector.md`
- Create: `website/src/content/docs/examples/data-modeling.md`

**Step 1: Create `website/src/content/docs/examples/agent-workflows.md`**

```markdown
---
title: Agent Workflows
description: End-to-end examples of AI agents using Teidelum
---

These examples show how an AI agent chains Teidelum's tools to answer complex questions.

## Find who's working on what

**User asks:** "Who is working on high-priority tasks?"

**Agent workflow:**

1. Query for high-priority tasks:
   ```json
   { "tool": "sql", "query": "SELECT title, assignee, status FROM project_tasks WHERE priority = 'high'" }
   ```

2. For each assignee, get their details:
   ```json
   { "tool": "sql", "query": "SELECT name, role, department FROM team_members WHERE name = 'Alice Chen'" }
   ```

3. Agent synthesizes: "Alice Chen (Backend Engineer) is working on 'Implement JWT rotation' (in progress) and Bob Smith (SRE) is handling 'Fix connection pool leak' (open)."

## Research a topic across all sources

**User asks:** "What do we know about rate limiting?"

**Agent workflow:**

1. Search for documents:
   ```json
   { "tool": "search", "query": "rate limiting", "limit": 5 }
   ```

2. Get related structured data:
   ```json
   { "tool": "sql", "query": "SELECT title, status FROM project_tasks WHERE title LIKE '%rate limit%'" }
   ```

3. Agent combines document content with task status for a complete picture.

## Trace an incident to its owner

**User asks:** "Who reported the API timeout incident and what else are they working on?"

**Agent workflow:**

1. Find the incident:
   ```json
   { "tool": "sql", "query": "SELECT description, reporter, severity FROM incidents WHERE description LIKE '%API timeout%'" }
   ```

2. Traverse the graph to find related entities:
   ```json
   {
     "tool": "graph",
     "table": "incidents",
     "key": "API gateway timeout affecting 5% of requests",
     "key_col": "description",
     "operation": "neighbors",
     "depth": 2,
     "direction": "both"
   }
   ```

3. Agent follows the graph from incident → reporter → their other tasks and incidents.

## Discover the data landscape

**User asks:** "What data do we have?"

**Agent workflow:**

1. Describe the catalog:
   ```json
   { "tool": "describe" }
   ```

2. Agent reads the table schemas and relationships, then explains what's available in natural language.
```

**Step 2: Create `website/src/content/docs/examples/custom-sync-source.md`**

```markdown
---
title: Custom Sync Source
description: Step-by-step guide to building a sync adapter
---

This example walks through building a sync source that pulls data from an external API.

## Step 1: Define the Struct

```rust
use anyhow::Result;
use async_trait::async_trait;
use teidelum::sync::{SearchDocument, StructuredRecord, SyncOutput, SyncSource};

pub struct MyAppSync {
    api_url: String,
    api_token: String,
}

impl MyAppSync {
    pub fn new(api_url: String, api_token: String) -> Self {
        Self { api_url, api_token }
    }
}
```

## Step 2: Implement the Trait

```rust
#[async_trait]
impl SyncSource for MyAppSync {
    fn name(&self) -> &str {
        "myapp"
    }

    async fn sync(
        &self,
        cursor: Option<&str>,
    ) -> Result<(SyncOutput, Option<String>)> {
        // 1. Fetch data from API (using cursor for incremental sync)
        let items = self.fetch_items(cursor).await?;

        let mut output = SyncOutput::default();

        for item in &items {
            // 2. Create structured record for SQL queries
            let mut fields = serde_json::Map::new();
            fields.insert("id".into(), item.id.clone().into());
            fields.insert("title".into(), item.title.clone().into());
            fields.insert("status".into(), item.status.clone().into());

            output.records.push(StructuredRecord {
                table: "myapp_items".into(),
                fields,
            });

            // 3. Create search document for full-text search
            output.documents.push(SearchDocument {
                id: format!("myapp-{}", item.id),
                source: "myapp".into(),
                title: item.title.clone(),
                body: item.description.clone(),
                metadata: serde_json::Map::new(),
            });
        }

        // 4. Return new cursor for next incremental sync
        let new_cursor = items.last().map(|i| i.updated_at.clone());

        Ok((output, new_cursor))
    }
}
```

## Step 3: Handle Incremental Sync

The cursor enables pulling only new/changed data:

```rust
impl MyAppSync {
    async fn fetch_items(&self, cursor: Option<&str>) -> Result<Vec<Item>> {
        let url = match cursor {
            Some(since) => format!("{}/items?updated_since={}", self.api_url, since),
            None => format!("{}/items", self.api_url),
        };
        // ... HTTP request and parsing
    }
}
```

## Key Points

- **Dual output**: Always produce both structured records (for SQL) and search documents (for full-text search) when the data supports it.
- **Incremental cursors**: Use timestamps, page tokens, or any opaque string that lets you resume from where you left off.
- **Idempotent**: Multiple syncs with the same cursor should produce the same result. The system handles deduplication at the storage layer.
```

**Step 3: Create `website/src/content/docs/examples/custom-connector.md`**

```markdown
---
title: Custom Connector
description: Step-by-step guide to building a live query adapter
---

This example walks through building a connector that queries an external database in real time.

## Step 1: Define the Struct

```rust
use anyhow::Result;
use async_trait::async_trait;
use teidelum::connector::{ColumnSchema, Connector, QueryResult, Value};

pub struct MyDbConnector {
    host: String,
    port: u16,
}

impl MyDbConnector {
    pub fn new(host: String, port: u16) -> Self {
        Self { host, port }
    }
}
```

## Step 2: Implement Table Discovery

```rust
#[async_trait]
impl Connector for MyDbConnector {
    fn name(&self) -> &str {
        "mydb"
    }

    async fn list_tables(&self) -> Result<Vec<(String, Vec<ColumnSchema>)>> {
        // Query the remote database for its schema
        Ok(vec![
            (
                "mydb_metrics".to_string(),
                vec![
                    ColumnSchema { name: "timestamp".into(), dtype: "timestamp".into() },
                    ColumnSchema { name: "metric".into(), dtype: "string".into() },
                    ColumnSchema { name: "value".into(), dtype: "f64".into() },
                ],
            ),
        ])
    }

    async fn query(&self, sql: &str) -> Result<QueryResult> {
        // Translate SQL to native query language and execute
        let native_query = self.translate_sql(sql)?;
        let raw_result = self.execute_native(&native_query).await?;
        self.convert_to_query_result(raw_result)
    }
}
```

## Step 3: SQL Translation

The hardest part of a connector is translating SQL to the source's native query language:

```rust
impl MyDbConnector {
    fn translate_sql(&self, sql: &str) -> Result<String> {
        // Parse the SQL and convert to your database's query format
        // This is source-specific — each database has its own language
        todo!("implement SQL translation")
    }
}
```

## Step 4: Register with Catalog

After implementing the connector, register its tables:

```rust
use teidelum::catalog::{TableEntry, ColumnInfo, StorageType};

// Register each table the connector exposes
api.register_table(TableEntry {
    name: "mydb_metrics".to_string(),
    source: "mydb".to_string(),
    storage: StorageType::Remote,  // Key: marks as remote
    columns: vec![
        ColumnInfo { name: "timestamp".into(), dtype: "timestamp".into() },
        ColumnInfo { name: "metric".into(), dtype: "string".into() },
        ColumnInfo { name: "value".into(), dtype: "f64".into() },
    ],
    row_count: None,  // Unknown for remote tables
});
```

## Key Differences from Sync Sources

| | Sync Source | Connector |
|---|---|---|
| **Data storage** | Copies data locally | Queries live, no local copy |
| **Latency** | Fast (local reads) | Depends on remote source |
| **Freshness** | As of last sync | Always current |
| **Use case** | Historical data, search | Real-time metrics, live queries |
```

**Step 4: Create `website/src/content/docs/examples/data-modeling.md`**

```markdown
---
title: Data Modeling
description: Design tables and relationships for effective graph queries
---

Good data modeling makes Teidelum's graph traversal and SQL queries more effective.

## Table Design

### Use string keys for graph traversal

The graph engine identifies nodes by a key column value. Use human-readable string keys:

```sql
-- Good: human-readable key
CREATE TABLE team_members (name VARCHAR, role VARCHAR, department VARCHAR)

-- Less useful for graph: numeric IDs require looking up the value
CREATE TABLE team_members (id BIGINT, name VARCHAR, role VARCHAR)
```

Both work, but string keys make graph results more readable and useful for AI agents.

### Keep tables focused

One table per entity type. Don't combine team members and tasks into a single table:

```
team_members: name, role, department
project_tasks: title, status, priority, assignee
incidents: description, severity, reporter
```

### Use consistent naming

- Table names: `snake_case`, plural (`team_members`, `project_tasks`)
- Column names: `snake_case` (`first_name`, `created_at`)
- FK columns: name should hint at the relationship (`assignee`, `reporter`)

## Relationship Design

### Model real-world connections

Each relationship should represent a meaningful real-world connection:

```rust
// Task → Person: who is responsible
Relationship {
    from_table: "project_tasks",
    from_col: "assignee",
    to_table: "team_members",
    to_col: "name",
    relation: "assigned_to",
}

// Incident → Person: who reported it
Relationship {
    from_table: "incidents",
    from_col: "reporter",
    to_table: "team_members",
    to_col: "name",
    relation: "reported_by",
}
```

### Relationship naming

The `relation` label should describe the edge in the **forward direction** (from → to):

- `assigned_to` (task → person)
- `reported_by` (incident → person)
- `belongs_to` (item → category)
- `depends_on` (task → task)

### Multiple relationships between tables

You can have multiple relationships between the same pair of tables:

```rust
// Tasks have both an assignee and a reviewer
Relationship { from_table: "tasks", from_col: "assignee", to_table: "people", to_col: "name", relation: "assigned_to" }
Relationship { from_table: "tasks", from_col: "reviewer", to_table: "people", to_col: "name", relation: "reviewed_by" }
```

Use `rel_types` filtering in graph queries to follow only specific relationship types.

## Graph Traversal Patterns

### Hub entities

Entities connected to many others (like team members) act as hubs. Querying neighbors of a hub with high depth returns a large result set. Use `depth: 1` for hubs.

### Chain traversal

For indirect relationships (task → assignee → other tasks), use `depth: 2` with directional filtering:

```json
{
  "table": "project_tasks",
  "key": "Implement JWT rotation",
  "key_col": "title",
  "depth": 2,
  "direction": "both"
}
```

This finds: task → assignee → other tasks assigned to the same person.
```

**Step 5: Commit**

```bash
git add website/src/content/docs/examples/
git commit -m "docs(website): add examples section"
```

---

### Task 11: GitHub Actions Deployment

**Files:**
- Create: `.github/workflows/deploy-website.yml`

**Step 1: Create `.github/workflows/deploy-website.yml`**

```yaml
name: Deploy Website

on:
  push:
    branches: [master]
    paths:
      - 'website/**'
      - '.github/workflows/deploy-website.yml'
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: true

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm
          cache-dependency-path: website/package-lock.json

      - name: Install dependencies
        working-directory: website
        run: npm ci

      - name: Build
        working-directory: website
        run: npm run build

      - name: Upload artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: website/dist

  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
```

**Step 2: Verify workflow syntax**

Run: `cat .github/workflows/deploy-website.yml | head -5`
Expected: Valid YAML, no syntax errors.

**Step 3: Commit**

```bash
git add .github/workflows/deploy-website.yml
git commit -m "ci: add GitHub Actions workflow for website deployment"
```

---

### Task 12: Final Verification

**Step 1: Run full build**

```bash
cd website && npm run build
```

Expected: Build succeeds, output in `website/dist/`.

**Step 2: Preview locally**

```bash
cd website && npm run preview
```

Expected: Visit `http://localhost:4321/`:
- Landing page renders with nav, hero, features, architecture, quick start, footer
- All links to docs work
- Sidebar navigation in docs is correct
- All 16 doc pages render without errors

**Step 3: Check CNAME is in build output**

```bash
cat website/dist/CNAME
```

Expected: `lum.teidedb.com`

**Step 4: Verify no broken internal links**

Click through all sidebar items in the preview. Every link should resolve.

**Step 5: Final commit (if any fixes needed)**

```bash
git add -A
git commit -m "fix(website): address build issues"
```
