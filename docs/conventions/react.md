# React Conventions — verwalter

> **Stack** (für dieses Projekt festgelegt):
> | Layer | Choice |
> |---|---|
> | Framework | React 19 |
> | Lang | TypeScript strict, siehe [typescript.md](typescript.md) |
> | Styling | Tailwind v4 Tokens + BEM, siehe [tailwind.md](tailwind.md) |
> | State | Zustand (nur flüchtiger UI-Zustand) |
> | Listen | virtualisiert mit `@tanstack/react-virtual`, siehe [ADR 002](../decisions/002-typgenerierung-und-listen.md) |
> | Markdown | `react-markdown` + `remark-gfm` + `rehype-highlight`, kein rohes HTML außer `<br>` (wird Zeilenumbruch) |
> | Diff | eigene Diff-Zeilen aus dem Core, virtualisiert ([ADR 006](../decisions/006-changes-und-diff.md)) |
>
> Projektentscheidungen in dieser Datei haben Vorrang vor allgemeinen Gewohnheiten.

## Project Layout

```text
src/
  app/                 Rahmen: Layout, Sidebar, Header
  components/          geteilte UI-Bausteine
  features/<feature>/  Komponenten + Hooks pro Feature (sessions, chat, changes, repositories, settings)
  stores/<feature>.ts  Zustand-Slices
  lib/                 Tauri-Wrapper, Hilfsfunktionen
  lib/bindings/        aus Rust generierte Typen (nicht von Hand ändern)
```

## Components

- Nur Funktionskomponenten
- Rückgabetyp explizit: `ReactElement` oder `ReactElement | null`
- Eine Komponente pro Datei, außer die innere ist winzig und nur von der äußeren benutzt
- Props-Interface über der Komponente, Name `<Component>Props`
- Komponente und ihre CSS-Datei liegen nebeneinander: `SessionList.tsx` + `SessionList.css`

## Fragments statt Wrapper-div

Mehrere Geschwister-Elemente → Fragment (`<>…</>`), kein umschließendes `<div>`. Ein echtes Wrapper-Element nur, wenn es Bedeutung trägt (`<article>`, `<section>`, `<nav>`) oder fürs Layout nötig ist.

## Hooks

- Am Anfang des Funktionskörpers, feste Reihenfolge: `useState`/`useReducer`, `useRef`, `useContext`, eigene Hooks, `useEffect`/`useLayoutEffect`
- Eigene Hooks beginnen mit `use`
- Dependency-Arrays vollständig — `eslint-plugin-react-hooks` ist aktiv und wird befolgt

## Conditional Rendering

Eine Bedingung in einer Zeile bleibt inline (`{isLoading && <Spinner />}`, ein flacher Ternary). Alles darüber hinaus — verschachtelt, mehrere Zweige, mehrzeiliges JSX pro Zweig — wandert in eine Render-Funktion:

```tsx
function renderBody(): ReactElement | null {
  if (isLoading) return <Spinner />;
  if (!session) return null;
  return <ChatTimeline sessionId={session.id} />;
}
```

## State

- Lokal: `useState` / `useReducer`. Nur hochziehen, wenn wirklich geteilt.
- Global: Zustand, ausschließlich für flüchtigen UI-Zustand (aktive Session, aktive Ansicht, gewähltes Repository, gewählte Datei).
- **Nie im Store:** komplette Event-Historie, kompletter Diff-Baum, Chat-Verlauf, Agent-Logs. Diese Daten liegen in SQLite und werden seitenweise über den Core nachgeladen.
- Core-Events (Tauri Events) aktualisieren gezielt, was sichtbar ist; sie füllen keinen globalen Spiegel der Datenbank.

## Effects

- `useEffect` synchronisiert mit Externem (Tauri-Event-Abos, DOM) — nicht für abgeleiteten Zustand
- Abgeleitetes → `useMemo` oder `const`
- Jedes Event-Abo gibt in der Cleanup-Funktion sein `unlisten` frei

## Performance

- Chat-Timeline, Dateilisten und Session-Liste werden virtualisiert — nur Sichtbares wird gerendert
- Der Diff kommt vom Core fertig zerlegt (Art, Nummern, Text) und wird wie jede lange Liste virtualisiert; höchstens 20 000 Zeilen je Datei
- Lange Tool-Ausgaben sind standardmäßig eingeklappt und werden erst beim Aufklappen geladen

## TypeScript specifics

Children: `ReactNode`. Events explizit typisieren (`React.ChangeEvent<HTMLTextAreaElement>` usw.). Alles Weitere in [typescript.md](typescript.md).

## Critical Rules

1. **Persistente Daten nie im React-/Zustand-State spiegeln** — sonst wächst der Speicher mit jeder Session und jedem Event, und das Performance-Ziel der App ist verfehlt.
2. **Lange Listen immer virtualisiert** — eine Chat-Historie mit tausenden Einträgen darf nicht vollständig im DOM landen.
3. **Kein direkter Zugriff auf Dateisystem, Git oder Prozesse aus React** — alles läuft über den Core.
4. **Jedes Event-Abo wird aufgeräumt** — vergessene Listener sind hier Speicherlecks pro Session.
