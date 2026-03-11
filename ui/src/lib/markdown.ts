import { Marked } from 'marked';
import DOMPurify from 'dompurify';
import type { HighlighterGeneric } from 'shiki';

let highlighterPromise: Promise<HighlighterGeneric<never, never>> | null = null;

function getHighlighter() {
	if (!highlighterPromise) {
		highlighterPromise = import('shiki').then((shiki) =>
			shiki.createHighlighter({
				themes: ['github-dark'],
				langs: [
					'javascript',
					'typescript',
					'python',
					'rust',
					'go',
					'java',
					'c',
					'cpp',
					'json',
					'yaml',
					'toml',
					'html',
					'css',
					'sql',
					'bash',
					'markdown',
					'diff'
				]
			})
		);
	}
	return highlighterPromise;
}

// Eagerly start loading the highlighter
getHighlighter();

/** Synchronous cache for highlighted code blocks */
const highlightCache = new Map<string, string>();

/** Queue a code block for async highlighting, returns placeholder or cached result */
function highlightCode(code: string, lang: string): string {
	const key = `${lang}:${code}`;
	const cached = highlightCache.get(key);
	if (cached) return cached;

	// Start async highlight
	getHighlighter().then((highlighter) => {
		const loadedLangs = highlighter.getLoadedLanguages();
		const effectiveLang = loadedLangs.includes(lang as never) ? lang : 'text';
		const html = highlighter.codeToHtml(code, {
			lang: effectiveLang,
			theme: 'github-dark'
		});
		highlightCache.set(key, html);
	});

	// Return plain code block as fallback until cache is populated
	const escaped = code.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
	return `<pre class="shiki"><code>${escaped}</code></pre>`;
}

// Use a local instance instead of modifying global marked config
const markedInstance = new Marked({
	breaks: true, // Convert \n to <br>
	gfm: true // GitHub-flavored markdown
});

// Custom renderer for code blocks with Shiki highlighting and copy button
markedInstance.use({
	renderer: {
		code({ text, lang }: { text: string; lang?: string }) {
			const language = lang || '';
			const highlighted = highlightCode(text, language);
			const escapedText = text
				.replace(/&/g, '&amp;')
				.replace(/</g, '&lt;')
				.replace(/>/g, '&gt;')
				.replace(/"/g, '&quot;');
			return `<div class="code-block-wrapper">${language ? `<span class="code-block-lang">${language}</span>` : ''}<button class="code-copy-btn" data-code="${escapedText}">Copy</button>${highlighted}</div>`;
		}
	}
});

/** Highlight @mentions in rendered HTML (after markdown, before sanitization) */
function highlightMentions(html: string): string {
	// Only highlight @mentions that are not inside HTML tags or code elements
	// Match @word outside of < > tags
	return html.replace(
		/(?<![<\w])@(\w+)(?![^<]*>)/g,
		'<span class="mention">@$1</span>'
	);
}

/** Render markdown text to sanitized HTML */
export function renderMarkdown(text: string): string {
	const html = markedInstance.parse(text, { async: false }) as string;
	const withMentions = highlightMentions(html);
	return DOMPurify.sanitize(withMentions, {
		ALLOWED_TAGS: [
			'p', 'br', 'strong', 'em', 'del', 'code', 'pre',
			'a', 'ul', 'ol', 'li', 'blockquote', 'h1', 'h2', 'h3',
			'span', 'div', 'button'
		],
		ALLOWED_ATTR: ['href', 'target', 'rel', 'class', 'data-code']
	});
}
