/** Der markierte Text, wenn die Auswahl nicht leer ist und beide Enden in `root` liegen; sonst `null`. */
export function selectedChatText(root: HTMLElement): string | null {
  const selection: Selection | null = window.getSelection();
  if (selection === null || selection.isCollapsed) {
    return null;
  }
  const { anchorNode, focusNode } = selection;
  if (
    anchorNode === null ||
    focusNode === null ||
    !root.contains(anchorNode) ||
    !root.contains(focusNode)
  ) {
    return null;
  }
  const text: string = selection.toString().trim();
  return text === '' ? null : text;
}
