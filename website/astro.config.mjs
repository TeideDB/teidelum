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
      social: {
        github: 'https://github.com/TeideDB/teidelum',
      },
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
