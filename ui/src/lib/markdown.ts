import { Marked } from 'marked';
import DOMPurify from 'dompurify';

// Use a local instance instead of modifying global marked config
const markedInstance = new Marked({
	breaks: true, // Convert \n to <br>
	gfm: true // GitHub-flavored markdown
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
			'span'
		],
		ALLOWED_ATTR: ['href', 'target', 'rel', 'class']
	});
}
