# Tailwind / Styling Conventions — verwalter

> **Stack** (für dieses Projekt festgelegt):
> | Layer | Choice |
> |---|---|
> | Tailwind | v4, CSS-first (`@theme`), über das Vite-Plugin |
> | Komponenten-Styles | eine CSS-Datei pro Komponente, BEM-Klassen, Verschachtelung über `postcss-nested` |
> | Themes | Hell / Dunkel / System |
>
> Quellen: [Tailwind-Doku](https://tailwindcss.com/docs) · [v4 Release Notes](https://tailwindcss.com/blog/tailwindcss-v4). Projektentscheidungen in dieser Datei haben Vorrang.

Zwei Arten von Regeln: **Upstream** = wie Tailwind v4 und die Plattform funktionieren. **Projektregel** = unsere Entscheidung.

## Upstream — wie v4 funktioniert

- `tailwindcss@^4` + `@tailwindcss/vite@^4` als devDependencies, Plugin in `vite.config.ts`
- Haupt-Stylesheet beginnt mit `@import "tailwindcss";`, gefolgt von einem `@theme { … }`-Block
- Keine `tailwind.config.js` — Konfiguration ist CSS
- Custom Properties in `@theme` werden echte CSS-Variablen **und** erzeugen passende Utility-Klassen (`--color-brand-500` → `bg-brand-500`). Welchen Kanal wir nutzen, ist Projektregel (unten).
- Dunkelmodus in reinem CSS: `@media (prefers-color-scheme: dark)` für die Systemeinstellung, `:root.dark` bzw. `:root.light` für die in den Settings erzwungene Wahl.

## Projektregel — Tokens in zwei Schichten

1. **Rohe Tokens** — Gestaltungsvokabular ohne Zweck: `--color-gray-700`, `--color-accent-500`, `--space-md`, `--font-sans`.
2. **Semantische Tokens** — nach Zweck benannt, verweisen auf rohe: `--color-bg-base: var(--color-gray-50)`, `--color-fg-primary`, `--color-border-subtle`.

**Komponenten verwenden nur semantische Tokens.** Hell/Dunkel tauscht ausschließlich semantische Tokens — ein Override-Block, null Änderungen an Komponenten.

Status-Farben tragen die Absicht, nicht den Farbton, und folgen dem Session-Status aus dem [Glossar](../glossary.md): `--color-status-running`, `--color-status-waiting`, `--color-status-error`, `--color-status-completed`.

### Token-Namen

- Sprechende Namen statt Zahlen: `--space-md` statt `--spacing-3`, `--z-modal` statt `--z-400`
- Abstände und Radien in T-Shirt-Größen (`2xs` … `2xl`)
- Z-Index nach Ebene: `base`, `dropdown`, `sticky`, `overlay`, `modal`, `toast`, `tooltip`
- Feste Präfixe: `--color-`, `--space-`, `--radius-`, `--shadow-`, `--z-`, `--duration-`, `--ease-`, `--font-`, `--font-size-`

## Projektregel — BEM + CSS-Datei pro Komponente, keine Utility-Klassen im JSX

Tailwind dient hier als **Token-Pipeline**. Die erzeugten Utility-Klassen werden nicht benutzt. JSX trägt BEM-Klassennamen, die das Ding beschreiben, nicht sein Aussehen; das Styling steht in der CSS-Datei neben der Komponente und liest Tokens über `var(--…)`.

Umgesetzt in `src/styles/theme.css`: `@import 'tailwindcss' source(none);` schaltet die Suche nach Klassennamen ab (sonst erzeugt Tailwind Utilities aus Beispielen in `docs/`), und `@theme static { … }` gibt jedes rohe Token als Variable aus, auch ohne Utility-Nutzung. Die semantischen Tokens stehen darunter in `:root` und in den Dunkel-Blöcken. Hex-Werte gibt es nur in dieser Datei.

**Verschachtelung:** `&__element` und `&--modifier` hängen an den Blocknamen an — das kann natives CSS-Nesting nicht (dort wird `&__name` zu einem kaputten Selektor, nicht zu `.block__name`). `postcss-nested` in `vite.config.ts` löst die Verschachtelung vor dem Bündeln auf. Ein Selektor mit zwei `&` (`&--x &__y`) steht ausgeschrieben unterhalb des Blocks (`.block--x .block__y`). Nach einer Änderung an CSS oder Build-Konfiguration: im gebauten CSS unter `dist/assets/` darf kein Selektor mit `__` oder `--` beginnen.

```tsx
// SessionListItem.tsx
<li className={`session-item ${isActive ? 'session-item--active' : ''}`}>
  <span className="session-item__name">{name}</span>
  <span className={`session-item__status session-item__status--${status}`} />
</li>
```

```css
/* SessionListItem.css */
.session-item {
  padding: var(--space-sm) var(--space-md);
  border-radius: var(--radius-md);

  &--active { background: var(--color-bg-selected); }
}

.session-item__status--waiting { color: var(--color-status-waiting); }
```

## Flacher Stil für Inhalte

Chat, Karten und Hinweise über dem Eingabefeld, die Popups des Dashboards, in der Übersicht des Vorhabens der Block „Fernsteuerung“, der Automatik-Block, die Session-Karten und das Intro „Neue Session“, die Liste der Repositories in „Neues Vorhaben“ und die Eingabesperre (Taiga 236, 255, 256; `docs/design/2026-11-25_chat-flach/`, `docs/design/2026-11-25_flacher-stil-reste/`):

- Keine runden Kästen mit eigener Fläche. Die Art eines Eintrags zeigt eine 2-px-Linie links (`border-left`) in ihrer Farbe.
- Listen sind Zeilen mit Haarlinien (`--color-border-subtle`).
- Code, Diffs, Fehlertexte und Felder sind eckig (`border-radius: 0`) mit Haarlinie; Knöpfe, Marken und Chips nehmen `--radius-flat` (2 px). Status- und Auswahlpunkte bleiben rund.
- Unter `src/features/chat/` gibt es kein `--radius-lg` und `--radius-xl`. In `RepositoryPicker.css`, `ProjectOverview.css`, `ProjectSessionCard.css` und `NewSessionIntro.css` und bei `.remote-block` und `.remote-lock` in `RemoteProject.css` gibt es auch kein `--radius-md`. Kästen dort sind eckig mit Haarlinie; die Sperre `.remote-lock` hat wie das Eingabefeld einen Rahmen mit `--radius-flat`, hier gestrichelt; der Block „Fernsteuerung“ (`.remote-block`) hat statt eines Rahmens links eine 2-px-Linie, im Akzent, solange ein Gerät das Vorhaben steuert (`remote-block--active`). Ein Radius von 2 px ist immer `var(--radius-flat)`, nie das Literal (Ausnahme: Balken und Symbole). Neue Bausteine werden gleich flach gebaut; der Stil gehört an die Quelle des Bausteins.
- Menüs, Dialoge und Popover außerhalb des Chats behalten ihre Rundung.

## Anti-Patterns

- ❌ Utility-Klassen im JSX (`className="px-8 flex text-sm"`) → BEM-Klasse + CSS
- ❌ Rohe Tokens direkt in Komponenten (`var(--color-accent-500)`) → semantisches Token
- ❌ Hex/RGB-Werte in Komponenten-CSS → Token
- ❌ `@apply` → `var(--token)` direkt
- ❌ Arbitrary-Value-Klassen (`bg-[#e8783a]`)
- ❌ `!important`
- ❌ Inline-`style` im JSX — Ausnahme: Werte, die sich zur Laufzeit ergeben (z.B. Positionen der virtualisierten Liste)
- ❌ Dauer/Easing pro Komponente hart kodiert → `var(--duration-base) var(--ease-out)`

## Critical Rules

1. **Komponenten lesen nur semantische Tokens** — sonst bricht Hell/Dunkel an jeder Stelle einzeln.
2. **Keine Utility-Klassen im JSX** — die Oberfläche bleibt ruhig und konsistent, weil alle Abstände und Farben aus einer Quelle kommen.
3. **Keine festen Farbwerte außerhalb von `@theme`.**
