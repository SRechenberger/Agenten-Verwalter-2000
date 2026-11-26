import { SESSION_REF_SCHEME } from '@/components/remarkSessionRefs';
import { unsourcedTip } from '@/features/sourcing/sourcingTexts';

// Bewusst schmale eigene Form statt der Typen aus `hast`: das Paket ist keine direkte Abhängigkeit
// und unter pnpm von hier aus nicht auflösbar. Jeder hast-Knoten ist dazu zuweisbar.
interface HastNode {
  type: string;
  value?: string;
  tagName?: string;
  properties?: Record<string, unknown>;
  children?: HastNode[];
}

interface RehypeUnsourcedOptions {
  needles: readonly string[];
}

const SKIPPED_TAG = 'pre';

// Ein Session-Link ist ein Knopf: eine ID darin ist keine Angabe, und eine fokussierbare Spanne
// gehört nicht in einen Knopf.
function isSessionRefLink(node: HastNode): boolean {
  const href: unknown = node.properties?.['href'];
  return node.tagName === 'a' && typeof href === 'string' && href.startsWith(SESSION_REF_SCHEME);
}

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

// Längste Nadel zuerst: bei gleicher Startstelle nimmt die Alternation die erste passende, so
// greift `src/a.ts:12` vor `12`.
function needlePattern(needles: readonly string[]): RegExp {
  const sorted: string[] = [...needles].sort(
    (left: string, right: string) => right.length - left.length,
  );
  return new RegExp(sorted.map(escapeRegExp).join('|'), 'g');
}

function markedSpan(needle: string): HastNode {
  return {
    type: 'element',
    tagName: 'span',
    properties: { className: ['unsourced'], tabIndex: 0, dataTip: unsourcedTip(needle) },
    children: [{ type: 'text', value: needle }],
  };
}

function splitText(value: string, pattern: RegExp): HastNode[] | null {
  const parts: HastNode[] = [];
  let cursor = 0;
  for (const match of value.matchAll(pattern)) {
    if (match.index > cursor) {
      parts.push({ type: 'text', value: value.slice(cursor, match.index) });
    }
    parts.push(markedSpan(match[0]));
    cursor = match.index + match[0].length;
  }
  if (cursor === 0) {
    return null;
  }
  if (cursor < value.length) {
    parts.push({ type: 'text', value: value.slice(cursor) });
  }
  return parts;
}

function markChildren(parent: HastNode, pattern: RegExp): void {
  if (parent.children === undefined) {
    return;
  }
  const marked: HastNode[] = [];
  for (const child of parent.children) {
    if (child.type === 'text' && child.value !== undefined) {
      marked.push(...(splitText(child.value, pattern) ?? [child]));
      continue;
    }
    if (child.type === 'element' && child.tagName !== SKIPPED_TAG && !isSessionRefLink(child)) {
      markChildren(child, pattern);
    }
    marked.push(child);
  }
  parent.children = marked;
}

/** Fasst jedes Vorkommen einer Nadel in einem Text-Knoten in ein `span.unsourced`; Codeblöcke
 *  (`pre`) und Links auf Sessions bleiben unberührt. Muss vor `rehype-highlight` laufen, solange dessen Spans noch fehlen. */
export function rehypeUnsourced(options: RehypeUnsourcedOptions): (tree: HastNode) => void {
  const needles: string[] = options.needles.filter((needle: string) => needle.length > 0);
  if (needles.length === 0) {
    return (): void => undefined;
  }
  const pattern: RegExp = needlePattern(needles);
  return (tree: HastNode): void => {
    markChildren(tree, pattern);
  };
}
