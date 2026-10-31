import { isValidElement, useContext, useMemo } from 'react';
import type { ReactElement, ReactNode } from 'react';
import ReactMarkdown, { defaultUrlTransform } from 'react-markdown';
import type { Components, Options } from 'react-markdown';
import rehypeHighlight from 'rehype-highlight';
import remarkGfm from 'remark-gfm';
import { common } from 'lowlight';
import powershell from 'highlight.js/lib/languages/powershell';
import { CodeBlock } from '@/components/CodeBlock';
import { ExternalLink } from '@/components/ExternalLink';
import { FileLink } from '@/components/FileLink';
import { FileLinkSessionContext } from '@/components/FileLinkSessionContext';
import { remarkBreakTags } from '@/components/remarkBreakTags';
import { rehypeUnsourced } from '@/features/sourcing/rehypeUnsourced';
import { filePathOf } from '@/lib/fileLinks';
import '@/features/sourcing/Unsourced.css';
import './Markdown.css';

interface MarkdownProps {
  text: string;
  /** Angaben, die als „ohne Fundstelle“ markiert werden; leer = keine Markierung. */
  unsourced?: readonly string[];
}

interface ScrollableTableProps {
  children?: ReactNode;
}

interface InlineCodeProps {
  className?: string | undefined;
  children?: ReactNode;
}

interface MarkdownLinkProps {
  href?: string | undefined;
  children?: ReactNode;
}

// Absoluter Windows-Pfad; der Backslash kommt aus mdast-util-to-hast als `%5C` an.
const WINDOWS_PATH_URL = /^[a-z]:(?:[\\/]|%5c)/i;
const FILE_URL = /^file:\/\/\//i;

const REMARK_PLUGINS: NonNullable<Options['remarkPlugins']> = [remarkGfm, remarkBreakTags];

// `common` kennt kein PowerShell; `detect: false`, damit nur Blöcke mit Sprachangabe gefärbt werden.
const REHYPE_PLUGINS: NonNullable<Options['rehypePlugins']> = [
  [rehypeHighlight, { detect: false, languages: { ...common, powershell } }],
];

const NO_NEEDLES: readonly string[] = [];

// Breite Tabellen scrollen in einer eigenen Hülle, statt den Chat aufzuweiten.
function ScrollableTable({ children }: ScrollableTableProps): ReactElement {
  return (
    <div className="markdown__table-scroll">
      <table>{children}</table>
    </div>
  );
}

// Der Text eines Inline-Codes; eine Marke teilt ihn in Strings und `span`-Elemente. `null`, sobald
// etwas anderes darin steckt.
function plainTextOf(node: ReactNode): string | null {
  if (typeof node === 'string') {
    return node;
  }
  if (Array.isArray(node)) {
    const parts: (string | null)[] = node.map((part: ReactNode) => plainTextOf(part));
    return parts.includes(null) ? null : parts.join('');
  }
  if (isValidElement<{ children?: ReactNode }>(node) && node.type === 'span') {
    return plainTextOf(node.props.children);
  }
  return null;
}

// Läuft auch für das `code` in Codeblöcken: deren Text endet immer auf `\n` (oder ist nach der
// Färbung kein einzelner String mehr) und wird so nie zum Dateiverweis.
function InlineCode({ className, children }: InlineCodeProps): ReactElement {
  const sessionId: string | null = useContext(FileLinkSessionContext);
  const code: ReactElement = <code className={className}>{children}</code>;
  const text: string | null = plainTextOf(children);
  if (sessionId === null || text === null || text.endsWith('\n')) {
    return code;
  }
  const path: string | null = filePathOf(text);
  if (path === null) {
    return code;
  }
  return (
    <FileLink sessionId={sessionId} path={path}>
      {code}
    </FileLink>
  );
}

function MarkdownLink({ href, children }: MarkdownLinkProps): ReactElement {
  const sessionId: string | null = useContext(FileLinkSessionContext);
  const path: string | null =
    sessionId === null || href === undefined ? null : filePathOf(decodeHref(href));
  if (sessionId === null || path === null) {
    return <ExternalLink href={href}>{children}</ExternalLink>;
  }
  return (
    <FileLink sessionId={sessionId} path={path}>
      {children}
    </FileLink>
  );
}

function decodeHref(href: string): string {
  try {
    return decodeURI(href);
  } catch {
    return href;
  }
}

// Ohne Ausnahme verwirft react-markdown `C:\…` (gilt ihm als unbekanntes Protokoll) und `file:///…`.
// Geöffnet wird beides nur über `FileLink`, also nach der Prüfung im Core.
function transformUrl(url: string): string {
  if (WINDOWS_PATH_URL.test(url) || FILE_URL.test(url)) {
    return url;
  }
  return defaultUrlTransform(url);
}

const COMPONENTS: Components = {
  pre: CodeBlock,
  code: InlineCode,
  a: MarkdownLink,
  table: ScrollableTable,
};

export function Markdown({ text, unsourced = NO_NEEDLES }: MarkdownProps): ReactElement {
  const rehypePlugins: NonNullable<Options['rehypePlugins']> = useMemo(
    () =>
      unsourced.length === 0
        ? REHYPE_PLUGINS
        : [[rehypeUnsourced, { needles: unsourced }], ...REHYPE_PLUGINS],
    [unsourced],
  );
  return (
    <div className="markdown">
      <ReactMarkdown
        remarkPlugins={REMARK_PLUGINS}
        rehypePlugins={rehypePlugins}
        components={COMPONENTS}
        urlTransform={transformUrl}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}
