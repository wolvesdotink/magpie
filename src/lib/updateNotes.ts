export type Inline =
  | { type: 'text'; value: string }
  | { type: 'bold'; value: string }
  | { type: 'link'; href: string; label: string };

export type NoteBlock =
  | { type: 'bullets'; items: Inline[][] }
  | { type: 'paragraph'; parts: Inline[] };

const URL_RE = /\bhttps?:\/\/[^\s<>]+/g;
const BOLD_RE = /\*\*([^*]+)\*\*/g;
const TRAILING_PUNCT = /[.,;:!?)\]]+$/;

function parseInline(line: string): Inline[] {
  const parts: Inline[] = [];
  const tokens: Array<{ start: number; end: number; node: Inline }> = [];

  for (const m of line.matchAll(BOLD_RE)) {
    const start = m.index ?? 0;
    tokens.push({ start, end: start + m[0].length, node: { type: 'bold', value: m[1] } });
  }
  for (const m of line.matchAll(URL_RE)) {
    const start = m.index ?? 0;
    let url = m[0];
    let trailingLen = 0;
    const trailing = url.match(TRAILING_PUNCT);
    if (trailing) {
      trailingLen = trailing[0].length;
      url = url.slice(0, -trailingLen);
    }
    tokens.push({
      start,
      end: start + url.length,
      node: { type: 'link', href: url, label: url },
    });
  }

  tokens.sort((a, b) => a.start - b.start);

  let cursor = 0;
  for (const tok of tokens) {
    if (tok.start < cursor) continue;
    if (tok.start > cursor) {
      parts.push({ type: 'text', value: line.slice(cursor, tok.start) });
    }
    parts.push(tok.node);
    cursor = tok.end;
  }
  if (cursor < line.length) {
    parts.push({ type: 'text', value: line.slice(cursor) });
  }
  return parts.length ? parts : [{ type: 'text', value: line }];
}

export function parseUpdateNotes(text: string | null | undefined): NoteBlock[] {
  if (!text) return [];

  const blocks: NoteBlock[] = [];
  const lines = text.replace(/\r\n/g, '\n').split('\n');

  let bullets: Inline[][] | null = null;
  let paragraph: string[] | null = null;

  const flushBullets = () => {
    if (bullets && bullets.length) blocks.push({ type: 'bullets', items: bullets });
    bullets = null;
  };
  const flushParagraph = () => {
    if (paragraph && paragraph.length) {
      blocks.push({ type: 'paragraph', parts: parseInline(paragraph.join(' ')) });
    }
    paragraph = null;
  };

  for (const raw of lines) {
    const line = raw.trim();
    if (!line) {
      flushBullets();
      flushParagraph();
      continue;
    }
    const bulletMatch = line.match(/^[-*]\s+(.*)$/);
    if (bulletMatch) {
      flushParagraph();
      if (!bullets) bullets = [];
      bullets.push(parseInline(bulletMatch[1]));
    } else {
      flushBullets();
      if (!paragraph) paragraph = [];
      paragraph.push(line);
    }
  }
  flushBullets();
  flushParagraph();

  return blocks;
}
