import { splitSessionRefs } from '@/lib/sessionRefs';
import type { SessionRefPart } from '@/lib/sessionRefs';

// Schmale eigene Form wie in `remarkBreakTags.ts`.
interface MdastNode {
  type: string;
  value?: string;
  url?: string;
  children?: MdastNode[];
}

/** Ziel-Schema der Links, die dieses Plugin setzt; `MarkdownLink` löst sie über den Kontext auf. */
export const SESSION_REF_SCHEME = 'session-ref:';

// Codeblöcke und Inline-Code haben `value` statt Kindern; Links bleiben, wie sie sind.
const OPAQUE = new Set(['link', 'linkReference', 'definition']);

function linkOf(part: SessionRefPart): MdastNode {
  if (part.kind === 'text') {
    return { type: 'text', value: part.text };
  }
  const target: string = part.kind === 'number' ? String(part.number) : part.id;
  return {
    type: 'link',
    url: `${SESSION_REF_SCHEME}${target}`,
    children: [{ type: 'text', value: part.text }],
  };
}

function linkRefs(parent: MdastNode): void {
  if (parent.children === undefined) {
    return;
  }
  parent.children = parent.children.flatMap((child: MdastNode): MdastNode[] => {
    if (child.type === 'text' && child.value !== undefined) {
      const parts: SessionRefPart[] = splitSessionRefs(child.value);
      return parts.length === 1 && parts[0]?.kind === 'text' ? [child] : parts.map(linkOf);
    }
    if (!OPAQUE.has(child.type)) {
      linkRefs(child);
    }
    return [child];
  });
}

/** Macht aus „Session N“ und Session-IDs im freien Text Links; ob es die Session gibt, entscheidet erst
 *  die Darstellung. */
export function remarkSessionRefs(): (tree: MdastNode) => void {
  return (tree: MdastNode): void => {
    linkRefs(tree);
  };
}
