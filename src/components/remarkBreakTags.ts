// Bewusst schmale eigene Form statt der Typen aus `mdast`: das Paket ist keine direkte Abhängigkeit
// und unter pnpm von hier aus nicht auflösbar. Jeder mdast-Knoten ist dazu zuweisbar.
interface MdastNode {
  type: string;
  value?: string;
  children?: MdastNode[];
}

const BREAK_TAG = /^<br\s*\/?>$/i;

function replaceBreakTags(parent: MdastNode): void {
  if (parent.children === undefined) {
    return;
  }
  parent.children = parent.children.map((child: MdastNode): MdastNode => {
    if (child.type === 'html' && child.value !== undefined && BREAK_TAG.test(child.value.trim())) {
      return { type: 'break' };
    }
    replaceBreakTags(child);
    return child;
  });
}

/** Macht aus `<br>` einen Zeilenumbruch; anderes rohes HTML bleibt Text. Tabellenzellen kennen
 *  in Markdown keinen anderen Umbruch, Agenten schreiben dort deshalb `<br>`. */
export function remarkBreakTags(): (tree: MdastNode) => void {
  return (tree: MdastNode): void => {
    replaceBreakTags(tree);
  };
}
