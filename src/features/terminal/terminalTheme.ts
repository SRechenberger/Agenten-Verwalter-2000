import type { ITheme } from '@xterm/xterm';

function tokenValue(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Farben des Terminals aus den Design-Tokens; xterm.js kennt keine CSS-Variablen, deshalb werden sie hier aufgelöst. */
export function terminalTheme(): ITheme {
  return {
    background: tokenValue('--color-bg-base'),
    foreground: tokenValue('--color-fg-primary'),
    cursor: tokenValue('--color-fg-primary'),
    cursorAccent: tokenValue('--color-bg-base'),
    selectionBackground: tokenValue('--color-accent-subtle'),
  };
}

export function terminalFont(): string {
  return tokenValue('--font-mono');
}
