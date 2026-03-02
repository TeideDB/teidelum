# Teidelum Website v2 — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a corporate website for Teidelum at lum.teidedb.com — vanilla landing page + 4-page docs section.

**Architecture:** Two-part static site. Landing page (`index.html`) is a single scrollable page with hero, features, architecture diagram, code example, and footer. Docs section (`docs/`) uses a sidebar + content layout with 4 pages. All vanilla HTML/CSS/JS, no build tools.

**Tech Stack:** HTML, CSS (custom properties), vanilla JS, GitHub Pages, GitHub Actions.

**Reference site:** `../org` (teidedb.com) — the org site's `style.css`, `index.html`, and `script.js` are the design system source of truth. Match its color palette, typography (Inter/Oswald/JetBrains Mono), component patterns (floating pill nav, feature cards, code blocks), and responsive breakpoints (1024/768/480px).

---

### Task 1: Scaffold directory and deployment files

**Files:**
- Create: `website/CNAME`
- Create: `website/.nojekyll`
- Modify: `.github/workflows/deploy-website.yml`

**Step 1: Create website directory and static deployment files**

```bash
mkdir -p website/assets website/docs
```

**Step 2: Create CNAME file**

Create `website/CNAME`:
```
lum.teidedb.com
```

**Step 3: Create .nojekyll file**

Create `website/.nojekyll` (empty file):
```bash
touch website/.nojekyll
```

**Step 4: Simplify GitHub Actions workflow for static files**

Replace `.github/workflows/deploy-website.yml` — remove Node.js/npm steps since we're serving static files directly:

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
  deploy:
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Upload artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: website

      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
```

**Step 5: Verify directory structure**

```bash
ls -la website/
```

Expected: `CNAME`, `.nojekyll`, `assets/`, `docs/` directories exist.

**Step 6: Commit**

```bash
git add website/CNAME website/.nojekyll .github/workflows/deploy-website.yml
git commit -m "chore: scaffold website directory and simplify deployment"
```

---

### Task 2: Create SVG logo assets

**Files:**
- Create: `website/assets/teidelum-logo.svg`
- Create: `website/assets/teidelum-icon.svg`
- Create: `website/assets/favicon.svg`

The Teidelum logo is a geometric mountain/lens abstraction. It shares the TeideDB mountain DNA but adds a lens aperture element — representing Teidelum's role as a "lens" on your data. Color: `#4b6777`.

**Step 1: Create icon-only SVG**

Create `website/assets/teidelum-icon.svg`. This is a mountain silhouette with a lens/aperture arc element:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 40" fill="none">
  <!-- Mountain peak -->
  <path d="M24 2 L6 34 H42 Z" fill="#4b6777"/>
  <!-- Lens aperture arc across the peak -->
  <path d="M10 26 Q24 18 38 26" stroke="#ffffff" stroke-width="2.5" fill="none" stroke-linecap="round"/>
  <!-- Light ray from lens -->
  <path d="M24 22 L24 34" stroke="#ffffff" stroke-width="1.5" stroke-linecap="round" opacity="0.6"/>
</svg>
```

**Step 2: Create full logo SVG (icon + wordmark)**

Create `website/assets/teidelum-logo.svg`. Mountain icon on the left, "TEIDELUM" text in Oswald Bold on the right:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 280 40" fill="none">
  <!-- Mountain peak -->
  <path d="M20 2 L2 34 H38 Z" fill="#4b6777"/>
  <!-- Lens aperture arc -->
  <path d="M6 26 Q20 18 34 26" stroke="#ffffff" stroke-width="2.5" fill="none" stroke-linecap="round"/>
  <!-- Light ray -->
  <path d="M20 22 L20 34" stroke="#ffffff" stroke-width="1.5" stroke-linecap="round" opacity="0.6"/>
  <!-- Wordmark -->
  <text x="48" y="29" fill="#4b6777" font-family="Oswald, sans-serif" font-weight="700" font-size="26" letter-spacing="0.08em">TEIDELUM</text>
</svg>
```

**Step 3: Create favicon**

Create `website/assets/favicon.svg`. Same as icon but optimized for small sizes — simpler geometry:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" fill="none">
  <path d="M16 2 L4 28 H28 Z" fill="#4b6777"/>
  <path d="M7 22 Q16 15 25 22" stroke="#ffffff" stroke-width="2" fill="none" stroke-linecap="round"/>
</svg>
```

**Step 4: Verify SVGs render correctly**

```bash
# Check files exist and are valid SVG
head -1 website/assets/teidelum-icon.svg
head -1 website/assets/teidelum-logo.svg
head -1 website/assets/favicon.svg
```

Expected: Each starts with `<svg`.

**Step 5: Commit**

```bash
git add website/assets/
git commit -m "feat(website): add Teidelum SVG logo, icon, and favicon"
```

---

### Task 3: Create shared CSS (design tokens + landing page styles)

**Files:**
- Create: `website/style.css`

This is the largest file. It must include: reset, design tokens (`:root` vars), typography, nav, hero, features, architecture, code example, footer, buttons, animations, responsive breakpoints. Copy the design system from `../org/style.css` but adapt section-specific styles for Teidelum's content.

**Step 1: Create `website/style.css`**

The file structure follows the org site exactly. Key sections:

1. **Reset + `:root` custom properties** — copy all color, font, and spacing tokens from `../org/style.css` lines 1-32 verbatim
2. **Base styles** (html, body, a, code, .container) — copy from org lines 34-71
3. **Nav** — copy `.nav`, `.nav-brand`, `.nav-links`, `.nav-toggle`, `.nav-github` blocks from org lines 73-133 verbatim (same floating pill pattern)
4. **Hero** — copy from org lines 134-180, but change content widths and headline size to match Teidelum's shorter headline
5. **Buttons** — copy `.btn`, `.btn-primary`, `.btn-outline`, `.btn-sm`, `.btn-ghost` from org lines 182-208 verbatim
6. **Section label** — copy from org line 211-215 verbatim
7. **Features** — use org's `.features-section` / `.features-grid` / `.feature-item` pattern (lines 217-246) but change grid to `repeat(3, 1fr)` for 6 cards (3x2 layout)
8. **Architecture section** — new section, styled like org's code section (centered content):
   ```css
   .arch-section { padding: 80px 0 100px; }
   .arch-section .section-label { text-align: center; }
   .arch-title {
     font-family: var(--font-heading); font-size: 2.5rem; font-weight: 700;
     text-align: center; margin-bottom: 8px; color: var(--text);
   }
   .arch-subtitle {
     text-align: center; color: var(--gray-text); margin-bottom: 48px; font-size: 1.05rem;
   }
   .arch-diagram {
     max-width: 900px; margin: 0 auto;
     background: var(--navy); border-radius: 16px; padding: 48px 32px;
     box-shadow: 0 24px 80px rgba(14,27,36,.12);
   }
   .arch-flow {
     display: flex; align-items: center; justify-content: center; gap: 24px;
     flex-wrap: wrap; color: #e2e8f0; font-family: var(--mono); font-size: .85rem;
   }
   .arch-node {
     background: rgba(255,255,255,.08); border: 1px solid rgba(255,255,255,.12);
     border-radius: 12px; padding: 16px 20px; text-align: center;
   }
   .arch-node-label {
     font-family: var(--font-heading); font-size: .7rem; font-weight: 600;
     text-transform: uppercase; letter-spacing: .1em; color: rgba(255,255,255,.4);
     margin-bottom: 8px;
   }
   .arch-node-items { display: flex; flex-direction: column; gap: 4px; }
   .arch-node-items span { color: #60a5fa; }
   .arch-arrow { color: var(--primary-light); font-size: 1.5rem; font-weight: 300; }
   .arch-core {
     background: rgba(75,103,119,.3); border: 1px solid rgba(75,103,119,.5);
     border-radius: 16px; padding: 20px 28px;
   }
   .arch-core-title {
     font-family: var(--font-heading); font-size: 1rem; font-weight: 700;
     color: var(--primary-light); margin-bottom: 12px; text-align: center;
   }
   .arch-core-tools {
     display: flex; gap: 12px;
   }
   .arch-tool {
     background: rgba(255,255,255,.06); border-radius: 8px; padding: 8px 14px;
     font-size: .8rem; color: #e2e8f0;
   }
   ```
9. **Code section** — copy from org lines 331-379 verbatim (code block, header, dots, syntax highlighting classes)
10. **Footer** — copy from org lines 416-425 verbatim
11. **Scroll-to-top** — copy from org lines 427-441 verbatim
12. **Animations** — copy from org lines 443-448 verbatim
13. **Responsive** — based on org lines 450-484, adapted:
    - At 1024px: features grid → `repeat(2, 1fr)`
    - At 768px: features/arch grid → `1fr`, mobile nav, smaller headings
    - At 480px: hero h1 → 2rem, smaller padding
14. **Utilities** — copy from org lines 486-492 verbatim

Write the complete `website/style.css` file with all sections above. Do NOT use placeholder comments like "add styles here" — write complete CSS for every selector.

**Step 2: Verify file is complete**

```bash
wc -l website/style.css
```

Expected: approximately 400-550 lines (org site is 493 lines).

**Step 3: Commit**

```bash
git add website/style.css
git commit -m "feat(website): add shared CSS with design tokens and landing page styles"
```

---

### Task 4: Create landing page HTML

**Files:**
- Create: `website/index.html`

Follow the org site's `index.html` structure exactly. Use semantic HTML, proper `<head>` meta tags, Google Fonts import, and all component patterns from the org site.

**Step 1: Create `website/index.html`**

Structure (follow org site's `index.html` as template — see `../org/index.html`):

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Teidelum — Local-First MCP Server</title>
  <meta name="description" content="Compact MCP server that syncs work tools and connects live data sources into a single searchable, queryable index. Single binary, zero config, data never leaves your machine.">
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&family=Oswald:wght@400;500;600;700&display=swap" rel="stylesheet">
  <link rel="stylesheet" href="style.css">
  <link rel="icon" href="assets/favicon.svg" type="image/svg+xml">
</head>
<body>
```

Sections to include (each following org site's HTML patterns):

1. **Nav** — Floating pill. Logo (28x28 icon SVG), "Teidelum" brand text in Oswald, nav links: Features | Docs (links to `docs/`) | Architecture | GitHub (links to `https://github.com/TeideDB/teidelum`). Hamburger toggle for mobile. Copy the exact `<nav>` structure from org `index.html` lines 17-40, changing brand name, links, and GitHub URL.

2. **Hero** — Cream background with decorative stripes (copy org hero-decor pattern). Large mountain SVG as background watermark (use Teidelum icon SVG path). Content:
   - H1: `Your tools. One index. <span class="accent">Local-first.</span>`
   - Chips: MCP Native, Full-Text Search, SQL Analytics, Graph Traversal, Zero Config
   - Subtitle: "Single binary MCP server that syncs work tools and connects live data sources into a single searchable, queryable index. Data never leaves your machine."
   - Buttons: [Get Started → docs/quick-start.html] [GitHub → repo URL with SVG icon]

3. **Features (#features)** — Section label "CAPABILITIES", 6 feature cards in a grid. Each card has: number (01-06), SVG icon, h3 title, paragraph. Cards:
   - 01: Full-Text Search — "BM25 ranking with fuzzy matching across all synced content. Highlighted snippets with source attribution and date filtering."
   - 02: SQL Analytics — "Columnar queries over structured data from any synced source. Filter, group, sort, join — all through a standard SQL interface."
   - 03: Graph Traversal — "Navigate foreign-key relationships between entities via BFS. Find neighbors, discover paths, filter by direction and relationship type."
   - 04: Work Tool Sync — "Pull data from Notion pages and Zulip messages with incremental sync. Only changed records on each run."
   - 05: Live Connectors — "Query external data sources in real-time. Built-in kdb+ adapter with a trait for adding your own connectors."
   - 06: Single Binary — "One Rust binary, no config files, no services to run. Set TEIDELUM_DATA and go. Everything stays local."

   Use SVG icons from org site (copy the `<svg viewBox="0 0 24 24">` inline icon pattern). Pick appropriate icons:
   - 01 (Search): magnifying glass / search icon
   - 02 (SQL): layers / database icon (reuse org's feature icon from line 84)
   - 03 (Graph): network/nodes icon (reuse org's from line 102)
   - 04 (Sync): refresh/arrows icon
   - 05 (Connectors): zap/lightning icon
   - 06 (Binary): package/box icon (reuse org's from line 111)

4. **Architecture (#architecture)** — Section label "ARCHITECTURE", title "How it works", subtitle about data flow. A styled diagram block (navy background, like code block) showing the data flow visually using styled HTML divs:
   ```
   [Sources]  →  [Teidelum: search | sql | graph]  →  [MCP Tools]  →  [AI Agents]
   ```
   Below the diagram: 1-2 sentences about dual storage (tantivy for full-text, libteide for columnar SQL).

5. **Code Example (#code)** — Section label "SEE IT IN ACTION", title "Connect and query", subtitle. Mac-style code block showing a JSON MCP tool call example. Use the org's code block HTML pattern (lines 239-268). Show a practical example — an agent calling the `search` tool then the `sql` tool:

   ```json
   // Agent calls search tool
   {
     "tool": "search",
     "params": {
       "query": "JWT token rotation",
       "limit": 5
     }
   }

   // Then queries structured data
   {
     "tool": "sql",
     "params": {
       "query": "SELECT assignee, status, priority FROM project_tasks WHERE status = 'in_progress' ORDER BY priority"
     }
   }
   ```

   Use syntax highlighting spans: `.kw` for keys, `.str` for strings, `.num` for numbers, `.cmt` for comments.

6. **Footer** — Copy org pattern (lines 330-340). Links: GitHub, TeideDB.com, Docs, MIT License. Copyright line: "Teidelum — Local-first MCP server. MIT License."

7. **Scroll-to-top button** — Copy from org (lines 343-345).

8. **Script tag** — `<script src="script.js"></script>` before `</body>`.

Write the complete HTML. Do NOT use placeholder comments — write every element with real content.

**Step 2: Verify file is complete and valid**

```bash
# Check file exists and has reasonable size
wc -l website/index.html
# Quick syntax check — ensure tags are balanced
grep -c '<section' website/index.html  # expect 4 (features, arch, code, no getstarted — that's in docs)
```

**Step 3: Commit**

```bash
git add website/index.html
git commit -m "feat(website): add landing page with hero, features, architecture, code example"
```

---

### Task 5: Create landing page JavaScript

**Files:**
- Create: `website/script.js`

Copy the org site's `script.js` (97 lines) nearly verbatim. It handles: scroll-triggered fade-in animations, mobile nav toggle, nav shadow on scroll + active link highlighting, copy button, scroll-to-top. The only change: the `'.nav-links a[href^="#"]'` selector works for hash links on the landing page; doc links (`docs/`) won't match and that's correct.

**Step 1: Create `website/script.js`**

Copy the full contents of `../org/script.js` verbatim into `website/script.js`. No modifications needed — the selectors target the same CSS class names.

Reference: `../org/script.js` lines 1-97.

**Step 2: Verify**

```bash
wc -l website/script.js
```

Expected: ~97 lines.

**Step 3: Open landing page in browser to verify**

```bash
open website/index.html
```

Verify: nav renders, hero section displays, feature cards animate on scroll, code block copy works, scroll-to-top appears.

**Step 4: Commit**

```bash
git add website/script.js
git commit -m "feat(website): add landing page interactions"
```

---

### Task 6: Create docs CSS

**Files:**
- Create: `website/docs/docs.css`

Docs-specific layout: imports shared `style.css`, adds two-column layout (fixed sidebar + content), sidebar navigation styles, active page indicator, content typography, prev/next nav, mobile sidebar toggle.

**Step 1: Create `website/docs/docs.css`**

```css
/* Import shared design tokens and base styles */
@import url('../style.css');

/* ── Docs Layout ────────────────────────────── */
.docs-layout {
  display: flex;
  min-height: 100vh;
  padding-top: 72px; /* space for top bar */
}

/* ── Docs Top Bar ───────────────────────────── */
.docs-topbar {
  position: fixed; top: 0; left: 0; right: 0; z-index: 100;
  height: 56px;
  display: flex; align-items: center; justify-content: space-between;
  padding: 0 24px;
  background: rgba(255,255,255,.95);
  border-bottom: 1px solid var(--border);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
}
.docs-topbar-left {
  display: flex; align-items: center; gap: 16px;
}
.docs-topbar-left a {
  display: flex; align-items: center; gap: 6px;
  color: var(--gray-text); font-size: .85rem; font-weight: 500;
  transition: color .2s;
}
.docs-topbar-left a:hover { color: var(--text); }
.docs-brand {
  display: flex; align-items: center; gap: 8px;
  color: var(--text); font-family: var(--font-heading);
  font-size: 1rem; font-weight: 700;
  letter-spacing: .03em; text-transform: uppercase;
}
.docs-brand img { width: 22px; height: 22px; }

/* ── Sidebar ────────────────────────────────── */
.docs-sidebar {
  width: 240px; min-width: 240px;
  padding: 32px 24px;
  border-right: 1px solid var(--border);
  position: fixed; top: 56px; bottom: 0; left: 0;
  overflow-y: auto;
  background: var(--surface-alt);
}
.sidebar-section { margin-bottom: 28px; }
.sidebar-section-label {
  font-family: var(--font-heading); font-size: .7rem; font-weight: 600;
  text-transform: uppercase; letter-spacing: .1em;
  color: var(--gray-text); margin-bottom: 8px; padding-left: 12px;
}
.sidebar-link {
  display: block; padding: 6px 12px; border-radius: 8px;
  font-size: .875rem; font-weight: 500; color: var(--gray-text);
  transition: color .15s, background .15s;
  margin-bottom: 2px;
}
.sidebar-link:hover {
  color: var(--text); background: rgba(75,103,119,.06);
}
.sidebar-link.active {
  color: var(--primary); background: var(--primary-bg);
  font-weight: 600;
}

/* ── Content Area ───────────────────────────── */
.docs-content {
  margin-left: 240px;
  flex: 1;
  max-width: 800px;
  padding: 48px 48px 80px;
}

/* ── Content Typography ─────────────────────── */
.docs-content h1 {
  font-family: var(--font-heading); font-size: 2.2rem; font-weight: 700;
  color: var(--text); margin-bottom: 8px; line-height: 1.15;
}
.docs-content .page-subtitle {
  font-size: 1.05rem; color: var(--gray-text); margin-bottom: 40px;
  line-height: 1.6;
}
.docs-content h2 {
  font-family: var(--font-heading); font-size: 1.5rem; font-weight: 600;
  color: var(--text); margin-top: 48px; margin-bottom: 16px;
  padding-bottom: 8px; border-bottom: 1px solid var(--border-light);
}
.docs-content h3 {
  font-family: var(--font-heading); font-size: 1.15rem; font-weight: 600;
  color: var(--text); margin-top: 32px; margin-bottom: 12px;
}
.docs-content p {
  color: var(--text-light); font-size: .95rem; line-height: 1.75;
  margin-bottom: 16px;
}
.docs-content ul, .docs-content ol {
  color: var(--text-light); font-size: .95rem; line-height: 1.75;
  margin-bottom: 16px; padding-left: 24px;
}
.docs-content li { margin-bottom: 6px; }
.docs-content strong { color: var(--text); font-weight: 600; }

/* ── Tables ─────────────────────────────────── */
.docs-content table {
  width: 100%; border-collapse: collapse; margin-bottom: 24px;
  font-size: .875rem;
}
.docs-content th {
  text-align: left; padding: 10px 14px;
  background: var(--surface-alt); font-weight: 600;
  color: var(--primary-dark); border-bottom: 1px solid var(--border);
  font-family: var(--font-heading); font-size: .75rem;
  text-transform: uppercase; letter-spacing: .06em;
}
.docs-content td {
  padding: 8px 14px; border-bottom: 1px solid var(--border-light);
  color: var(--text-light);
}
.docs-content tr:last-child td { border-bottom: none; }
.docs-content tr:hover td { background: var(--primary-bg); }

/* ── Code Blocks in Docs ────────────────────── */
.docs-content pre {
  background: var(--navy); color: #e2e8f0;
  padding: 20px; border-radius: 12px;
  font-family: var(--mono); font-size: .82rem;
  line-height: 1.7; overflow-x: auto;
  margin-bottom: 24px;
}
.docs-content pre code {
  background: none; padding: 0; border-radius: 0;
  font-size: inherit; color: inherit;
}

/* ── Prev/Next Navigation ───────────────────── */
.docs-nav-footer {
  display: flex; justify-content: space-between; gap: 16px;
  margin-top: 64px; padding-top: 24px;
  border-top: 1px solid var(--border);
}
.docs-nav-link {
  display: flex; flex-direction: column; gap: 4px;
  padding: 16px 20px; border-radius: 12px;
  border: 1px solid var(--border); flex: 1;
  transition: border-color .2s, box-shadow .2s;
}
.docs-nav-link:hover {
  border-color: var(--primary-pale);
  box-shadow: 0 4px 16px rgba(75,103,119,.08);
}
.docs-nav-link.next { text-align: right; }
.docs-nav-label {
  font-size: .75rem; color: var(--gray-text); font-weight: 500;
  text-transform: uppercase; letter-spacing: .06em;
}
.docs-nav-title {
  font-family: var(--font-heading); font-size: 1rem; font-weight: 600;
  color: var(--primary);
}

/* ── Mobile Sidebar Toggle ──────────────────── */
.docs-sidebar-toggle {
  display: none; background: none; border: none; cursor: pointer;
  padding: 4px; color: var(--text);
}
.docs-sidebar-toggle svg { width: 20px; height: 20px; }

/* ── Responsive ─────────────────────────────── */
@media (max-width: 1024px) {
  .docs-content { padding: 36px 32px 60px; }
}
@media (max-width: 768px) {
  .docs-sidebar-toggle { display: block; }
  .docs-sidebar {
    display: none;
    position: fixed; top: 56px; left: 0; right: 0; bottom: 0;
    width: 100%; z-index: 99;
    background: rgba(255,255,255,.98);
    backdrop-filter: blur(12px);
  }
  .docs-sidebar.open { display: block; }
  .docs-content {
    margin-left: 0; padding: 24px 16px 60px;
  }
  .docs-content h1 { font-size: 1.8rem; }
  .docs-content h2 { font-size: 1.3rem; }
  .docs-nav-footer { flex-direction: column; }
}
```

**Step 2: Verify**

```bash
wc -l website/docs/docs.css
```

Expected: ~180-200 lines.

**Step 3: Commit**

```bash
git add website/docs/docs.css
git commit -m "feat(website): add docs-specific CSS layout"
```

---

### Task 7: Create docs JavaScript

**Files:**
- Create: `website/docs/docs.js`

Handles: mobile sidebar toggle, active page highlighting (based on current URL), code copy buttons.

**Step 1: Create `website/docs/docs.js`**

```javascript
document.addEventListener('DOMContentLoaded', () => {
  'use strict';

  // Mobile sidebar toggle
  const toggle = document.querySelector('.docs-sidebar-toggle');
  const sidebar = document.querySelector('.docs-sidebar');
  if (toggle && sidebar) {
    toggle.addEventListener('click', () => {
      sidebar.classList.toggle('open');
    });
    // Close sidebar when a link is clicked (mobile)
    sidebar.querySelectorAll('.sidebar-link').forEach((link) => {
      link.addEventListener('click', () => {
        sidebar.classList.remove('open');
      });
    });
  }

  // Highlight active page in sidebar based on current filename
  const currentPage = window.location.pathname.split('/').pop() || 'index.html';
  document.querySelectorAll('.sidebar-link').forEach((link) => {
    const href = link.getAttribute('href');
    if (href === currentPage || (currentPage === '' && href === 'index.html')) {
      link.classList.add('active');
    }
  });

  // Copy buttons for code blocks
  document.querySelectorAll('.copy-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      const codeBlock = btn.closest('.code-block') || btn.parentElement;
      const code = codeBlock.querySelector('code') || codeBlock.querySelector('pre');
      if (!code) return;
      navigator.clipboard.writeText(code.textContent).then(() => {
        btn.classList.add('copied');
        const original = btn.innerHTML;
        btn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg> Copied!';
        setTimeout(() => {
          btn.classList.remove('copied');
          btn.innerHTML = original;
        }, 2000);
      });
    });
  });
});
```

**Step 2: Verify**

```bash
wc -l website/docs/docs.js
```

Expected: ~45-50 lines.

**Step 3: Commit**

```bash
git add website/docs/docs.js
git commit -m "feat(website): add docs JavaScript interactions"
```

---

### Task 8: Create docs index page

**Files:**
- Create: `website/docs/index.html`

The docs hub / table of contents. Lists all 4 doc pages with brief descriptions, grouped by category.

**Step 1: Create `website/docs/index.html`**

Use the docs layout: topbar + sidebar + content area. The sidebar appears on all doc pages with the same structure. The content area shows a welcome message and links to all pages.

HTML structure:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Documentation — Teidelum</title>
  <meta name="description" content="Teidelum documentation — guides, reference, and examples for the local-first MCP server.">
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&family=Oswald:wght@400;500;600;700&display=swap" rel="stylesheet">
  <link rel="stylesheet" href="docs.css">
  <link rel="icon" href="../assets/favicon.svg" type="image/svg+xml">
</head>
<body>
  <!-- Top Bar -->
  <div class="docs-topbar">
    <div class="docs-topbar-left">
      <button class="docs-sidebar-toggle" aria-label="Toggle sidebar">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="18" x2="21" y2="18"/></svg>
      </button>
      <a href="../" class="docs-brand">
        <img src="../assets/teidelum-icon.svg" alt="Teidelum" width="22" height="22">
        Teidelum
      </a>
      <a href="../">← Back to home</a>
    </div>
    <a href="https://github.com/TeideDB/teidelum" class="nav-github" target="_blank" rel="noopener">
      <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>
      GitHub
    </a>
  </div>

  <div class="docs-layout">
    <!-- Sidebar (identical on all doc pages) -->
    <nav class="docs-sidebar">
      <div class="sidebar-section">
        <div class="sidebar-section-label">Getting Started</div>
        <a href="quick-start.html" class="sidebar-link">Quick Start</a>
      </div>
      <div class="sidebar-section">
        <div class="sidebar-section-label">Reference</div>
        <a href="mcp-tools.html" class="sidebar-link">MCP Tools</a>
      </div>
      <div class="sidebar-section">
        <div class="sidebar-section-label">Concepts</div>
        <a href="architecture.html" class="sidebar-link">Architecture</a>
      </div>
      <div class="sidebar-section">
        <div class="sidebar-section-label">Examples</div>
        <a href="examples.html" class="sidebar-link">Agent Workflows</a>
      </div>
    </nav>

    <!-- Content -->
    <main class="docs-content">
      <h1>Documentation</h1>
      <p class="page-subtitle">Everything you need to connect your tools and start querying.</p>

      <h2>Getting Started</h2>
      <p><a href="quick-start.html"><strong>Quick Start</strong></a> — Install, run the server, connect from an AI agent, and make your first query in under 5 minutes.</p>

      <h2>Reference</h2>
      <p><a href="mcp-tools.html"><strong>MCP Tools</strong></a> — Complete reference for all 5 MCP tools: search, sql, describe, graph, and sync. Parameters, return formats, and examples.</p>

      <h2>Concepts</h2>
      <p><a href="architecture.html"><strong>Architecture</strong></a> — How Teidelum works: module map, data flow, dual storage, query routing, and the catalog system.</p>

      <h2>Examples</h2>
      <p><a href="examples.html"><strong>Agent Workflows</strong></a> — End-to-end examples showing how AI agents chain search, SQL, and graph tools together.</p>
    </main>
  </div>

  <script src="docs.js"></script>
</body>
</html>
```

**Step 2: Verify**

```bash
wc -l website/docs/index.html
```

Expected: ~85-100 lines.

**Step 3: Commit**

```bash
git add website/docs/index.html
git commit -m "feat(website): add docs index page"
```

---

### Task 9: Create Quick Start page

**Files:**
- Create: `website/docs/quick-start.html`

Content: Prerequisites, build from source, run the server, connect from Claude Desktop, first query.

**Step 1: Create `website/docs/quick-start.html`**

Same `<head>`, topbar, and sidebar as `docs/index.html`. Content area covers:

**## Prerequisites**
- Rust toolchain (1.75+): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

**## Build from source**
```bash
git clone https://github.com/TeideDB/teidelum.git
cd teidelum
cargo build --release
```
The binary is at `target/release/teidelum`.

**## Run the server**
```bash
./target/release/teidelum
```
Teidelum serves MCP over stdio. On first run, it generates demo data (team members, project tasks, incidents) so you can start querying immediately.

Set `TEIDELUM_DATA` to control where data is stored (defaults to `./data`).

**## Connect from Claude Desktop**
Add to your Claude Desktop MCP config (`~/Library/Application Support/Claude/claude_desktop_config.json`):
```json
{
  "mcpServers": {
    "teidelum": {
      "command": "/path/to/teidelum"
    }
  }
}
```

**## Your first queries**
Once connected, try these in your agent:
- "Search for JWT token rotation" → triggers the `search` tool
- "Show me all high-priority tasks" → triggers the `sql` tool
- "Who is assigned to the most tasks?" → triggers `sql` with GROUP BY
- "What's related to Alice Chen?" → triggers the `graph` tool

**Prev/Next nav**: ← Docs Home | MCP Tools →

Write the complete HTML file.

**Step 2: Verify**

```bash
wc -l website/docs/quick-start.html
```

Expected: ~160-200 lines.

**Step 3: Commit**

```bash
git add website/docs/quick-start.html
git commit -m "docs(website): add Quick Start guide"
```

---

### Task 10: Create MCP Tools Reference page

**Files:**
- Create: `website/docs/mcp-tools.html`

This is the most content-dense page. Documents all 5 MCP tools with parameter tables, return formats, and JSON examples. Source of truth: `src/mcp.rs` parameter structs.

**Step 1: Create `website/docs/mcp-tools.html`**

Same layout as other doc pages. Content covers each tool:

**## search**
Description: "Full-text search across all connected sources"

Parameter table:
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| query | string | yes | — | Full-text search query |
| sources | string[] | no | all | Filter to specific sources |
| limit | number | no | 10 | Max results to return |
| date_from | string | no | — | Filter from date (ISO 8601) |
| date_to | string | no | — | Filter to date (ISO 8601) |

Example JSON:
```json
{
  "query": "JWT token rotation",
  "sources": ["notion"],
  "limit": 5
}
```

**## sql**
Description: "Run analytical queries over structured data from all sources"

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| query | string | yes | SQL query to execute |

Example:
```sql
SELECT assignee, COUNT(*) as task_count
FROM project_tasks
WHERE status = 'in_progress'
GROUP BY assignee
ORDER BY task_count DESC
```

**## describe**
Description: "List available tables, schemas, and relationships"

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| source | string | no | Filter to specific source |

**## graph**
Description: "Traverse relationships between entities (neighbors, paths)"

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| table | string | yes | — | Starting node table |
| key | string | yes | — | Node identifier value |
| key_col | string | no | "name" | Column to match key against |
| operation | string | no | "neighbors" | "neighbors" or "path" |
| depth | number | no | 2 | Max traversal depth (max: 10) |
| direction | string | no | "both" | "forward", "reverse", or "both" |
| rel_types | string[] | no | all | Filter by relationship types |
| to_table | string | path only | — | Target table (path operation) |
| to_key | string | path only | — | Target key (path operation) |
| to_key_col | string | no | key_col | Target key column (path operation) |

Neighbors example:
```json
{
  "table": "team_members",
  "key": "Alice Chen",
  "operation": "neighbors",
  "depth": 2
}
```

Path example:
```json
{
  "table": "team_members",
  "key": "Alice Chen",
  "operation": "path",
  "to_table": "incidents",
  "to_key": "API Gateway Timeout"
}
```

**## sync**
Description: "Trigger incremental sync for connected sources"

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| source | string | no | Sync specific source, or omit for all |

**Prev/Next nav**: ← Quick Start | Architecture →

Write the complete HTML file with all parameter tables using the `<table>` styling from docs.css.

**Step 2: Verify**

```bash
wc -l website/docs/mcp-tools.html
```

Expected: ~300-400 lines (this is the longest doc page).

**Step 3: Commit**

```bash
git add website/docs/mcp-tools.html
git commit -m "docs(website): add MCP Tools reference page"
```

---

### Task 11: Create Architecture page

**Files:**
- Create: `website/docs/architecture.html`

Content: Module overview, data flow, key design patterns.

**Step 1: Create `website/docs/architecture.html`**

Same layout. Content covers:

**## Module Overview**

Table (from CLAUDE.md):
| Module | Role |
|--------|------|
| `main.rs` | Entrypoint: opens TeidelumApi, registers relationships, serves MCP over stdio |
| `api.rs` | Unified API: wraps catalog, search, router, graph behind thread-safe interface |
| `mcp.rs` | MCP tool definitions via rmcp; delegates to TeidelumApi |
| `router.rs` | Query router: dispatches SQL to libteide (local) or connectors (remote) |
| `search.rs` | tantivy wrapper: BM25 + fuzzy search engine |
| `catalog.rs` | Metadata catalog: schemas, FK relationships, local vs remote tracking |
| `graph.rs` | SQL-based graph traversal: BFS over catalog FK relationships |
| `connector/kdb.rs` | kdb+ live query adapter |
| `sync/notion.rs` | Notion incremental sync |
| `sync/zulip.rs` | Zulip incremental sync |

**## Data Flow**

Text description + styled flow diagram (reuse the arch-diagram CSS from the landing page, or a simpler text-based flow):

1. **Sync sources** pull data from Notion/Zulip
2. Data splits into **structured fields** → libteide columnar tables (for SQL) and **freeform content** → tantivy full-text index (for search)
3. **Catalog** tracks all table schemas and FK relationships
4. **Query router** inspects catalog to dispatch: local tables → libteide, remote → connectors
5. **Graph engine** uses catalog FK relationships to traverse between entities via SQL queries at each hop
6. **MCP tools** wrap everything: search, sql, describe, graph, sync

**## Key Design Patterns**

Brief sections on:
- **Dual Storage**: Why structured and freeform are separated
- **Catalog-Driven Routing**: How the router knows where to send queries
- **Incremental Sync**: Cursor tracking for efficient updates
- **Unified API**: TeidelumApi as the single facade with RwLock for concurrent reads

**Prev/Next nav**: ← MCP Tools | Examples →

**Step 2: Verify**

```bash
wc -l website/docs/architecture.html
```

Expected: ~200-250 lines.

**Step 3: Commit**

```bash
git add website/docs/architecture.html
git commit -m "docs(website): add Architecture overview page"
```

---

### Task 12: Create Examples page

**Files:**
- Create: `website/docs/examples.html`

Content: End-to-end agent workflows showing practical tool chaining.

**Step 1: Create `website/docs/examples.html`**

Same layout. Content covers:

**## Investigation Workflow: Search → SQL → Graph**

Scenario: "An agent needs to understand who's working on authentication issues and what's related."

Step 1 — Search for context:
```json
{ "tool": "search", "params": { "query": "authentication JWT" } }
```
Returns: matching documents from Notion/Zulip with highlighted snippets.

Step 2 — Query structured data:
```json
{ "tool": "sql", "params": { "query": "SELECT name, status, priority, assignee FROM project_tasks WHERE name LIKE '%JWT%' OR name LIKE '%auth%'" } }
```
Returns: table of matching tasks with assignees.

Step 3 — Explore relationships:
```json
{ "tool": "graph", "params": { "table": "team_members", "key": "Alice Chen", "operation": "neighbors", "depth": 2, "direction": "reverse" } }
```
Returns: all entities connected to Alice — her tasks, incidents she reported.

**## Discovery Workflow: Describe → SQL**

Scenario: "Agent doesn't know what data is available."

Step 1 — Discover schema:
```json
{ "tool": "describe", "params": {} }
```

Step 2 — Query based on discovered tables:
```json
{ "tool": "sql", "params": { "query": "SELECT team, COUNT(*) FROM team_members GROUP BY team" } }
```

**## Path Finding: Graph Connections**

Scenario: "Find the connection between a person and an incident."

```json
{
  "tool": "graph",
  "params": {
    "table": "team_members",
    "key": "Alice Chen",
    "operation": "path",
    "to_table": "incidents",
    "to_key": "API Gateway Timeout",
    "depth": 4
  }
}
```

**Prev/Next nav**: ← Architecture | (none — last page)

**Step 2: Verify**

```bash
wc -l website/docs/examples.html
```

Expected: ~200-250 lines.

**Step 3: Commit**

```bash
git add website/docs/examples.html
git commit -m "docs(website): add Agent Workflows examples page"
```

---

### Task 13: Final verification and commit

**Files:**
- All files in `website/`

**Step 1: Verify complete directory structure**

```bash
find website -type f | sort
```

Expected:
```
website/.nojekyll
website/CNAME
website/assets/favicon.svg
website/assets/teidelum-icon.svg
website/assets/teidelum-logo.svg
website/docs/architecture.html
website/docs/docs.css
website/docs/docs.js
website/docs/examples.html
website/docs/index.html
website/docs/mcp-tools.html
website/docs/quick-start.html
website/index.html
website/script.js
website/style.css
```

Total: 15 files.

**Step 2: Verify all HTML files have valid structure**

```bash
# Check each HTML file has DOCTYPE, html, head, body tags
for f in website/index.html website/docs/*.html; do
  echo "=== $f ==="
  grep -c '<!DOCTYPE html>' "$f"
  grep -c '</html>' "$f"
done
```

Expected: each file shows `1` for both counts.

**Step 3: Open landing page in browser for visual verification**

```bash
open website/index.html
```

Check:
- Nav renders with logo, links, GitHub button
- Hero section: headline, chips, subtitle, CTA buttons
- Features: 6 cards in 3x2 grid with icons
- Architecture: navy diagram block
- Code example: mac-style code block with syntax highlighting
- Footer with links
- Scroll-to-top button appears after scrolling

**Step 4: Open docs in browser**

```bash
open website/docs/index.html
```

Check:
- Top bar with back link and GitHub button
- Sidebar with 4 sections
- Content area with links to all pages
- Click through each page — sidebar highlights active page
- Prev/next navigation works
- Code blocks render with navy background
- Tables render with proper styling
- Mobile: sidebar collapses behind hamburger

**Step 5: Verify all internal links work**

```bash
# Check that all href targets exist as files
grep -roh 'href="[^"]*\.html"' website/ | sort -u
```

Manually verify each link points to an existing file.

**Step 6: Final commit if any fixes were needed**

```bash
git add website/
git commit -m "feat(website): complete Teidelum corporate website v2"
```
