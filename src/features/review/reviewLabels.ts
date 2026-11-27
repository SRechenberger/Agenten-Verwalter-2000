import { displayFile } from '@/features/artifacts/groupArtifacts';
import type { DiffLineKind } from '@/lib/bindings/DiffLineKind';
import type { CommentTarget } from '@/lib/bindings/CommentTarget';
import type { ReviewComment } from '@/lib/bindings/ReviewComment';

/** „Zeile N“; bei einer gelöschten Zeile ist N die alte Nummer: „Zeile N (alt)“. */
export function lineLabel(kind: DiffLineKind, line: number): string {
  if (kind === 'deleted') {
    return `Zeile ${String(line)} (alt)`;
  }
  return `Zeile ${String(line)}`;
}

/** Teil des Pfads nach dem letzten `/`. */
export function fileName(path: string): string {
  return path.slice(path.lastIndexOf('/') + 1);
}

/** Ortsangabe einer Kommentar-Karte: „<Repository> / <Pfad> · Zeile N“. */
export function whereLabel(comment: ReviewComment): string {
  switch (comment.target.kind) {
    case 'diff':
      return `${comment.repositoryName} / ${comment.path} · ${lineLabel(comment.kind, comment.line)}`;
    case 'artifact':
      return `${displayFile(comment.target.file)} · ${selectorTail(comment.target.selector)} „${comment.target.label}“`;
    case 'image':
      return `${comment.target.name} · Bereich ${String(comment.target.width)} × ${String(comment.target.height)} bei ${String(comment.target.x)}, ${String(comment.target.y)}`;
  }
}

export function kindLabel(target: CommentTarget): string {
  switch (target.kind) {
    case 'diff':
      return 'Diff';
    case 'artifact':
      return 'Artefakt';
    case 'image':
      return 'Bild';
  }
}

export function selectorTail(selector: string): string {
  const parts: string[] = selector.split('>');
  const tail: string | undefined = parts[parts.length - 1];
  return (tail ?? selector).trim();
}
