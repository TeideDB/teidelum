import { marked } from 'marked';
import DOMPurify from 'dompurify';

// Configure marked for chat messages
marked.setOptions({
	breaks: true, // Convert \n to <br>
	gfm: true // GitHub-flavored markdown
});

/** Render markdown text to sanitized HTML */
export function renderMarkdown(text: string): string {
	const html = marked.parse(text, { async: false }) as string;
	return DOMPurify.sanitize(html, {
		ALLOWED_TAGS: [
			'p', 'br', 'strong', 'em', 'del', 'code', 'pre',
			'a', 'ul', 'ol', 'li', 'blockquote', 'h1', 'h2', 'h3'
		],
		ALLOWED_ATTR: ['href', 'target', 'rel']
	});
}
