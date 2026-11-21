import { useEffect, useState } from 'react';
import type { ToolEntry } from '@/features/chat/buildBlocks';
import { parseSessionLinks, readsSessionLinks, type ToolLink } from '@/features/chat/sessionLinks';
import { loadToolDetail } from '@/lib/toolDetails';

const NONE: readonly ToolLink[] = [];

interface Loaded {
  key: string;
  links: readonly ToolLink[];
}

/** Die Ziele eines Session-Werkzeug-Aufrufs, gelesen aus seiner Ausgabe, ohne dass die Zeile aufgeklappt ist.
 *  Geladen wird einmal, sobald der Aufruf fertig ist; ein Fehler lässt die Zeile, wie sie ist. */
export function useToolLinks(sessionId: string, tool: ToolEntry): readonly ToolLink[] {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const key = `${sessionId}:${tool.toolUseId}`;
  const isWanted: boolean = tool.state === 'done' && readsSessionLinks(tool.tool);
  const { toolUseId, tool: name } = tool;

  useEffect(() => {
    if (!isWanted) {
      return;
    }
    let isCancelled = false;
    loadToolDetail(sessionId, toolUseId)
      .then((detail) => {
        if (!isCancelled) {
          setLoaded({ key, links: detail.missing ? NONE : parseSessionLinks(name, detail.output) });
        }
      })
      .catch((reason: unknown) => {
        console.error('Ziele des Werkzeug-Aufrufs nicht ladbar', reason);
      });
    return (): void => {
      isCancelled = true;
    };
  }, [sessionId, toolUseId, name, key, isWanted]);

  return loaded?.key === key ? loaded.links : NONE;
}
