# 044 — Nebenfragen: Fork der Session mit gleicher Werkzeugliste

**Status:** angenommen · **Datum:** 2026-10-29

## Kontext

Der Nutzer will eine Frage zur laufenden Session stellen, die Claude aus dem bisherigen Verlauf beantwortet, ohne dass Frage oder Antwort in den Chat wandern und ohne den laufenden Agenten zu stören (Vorbild `/btw` in Claude Code). Je Session läuft genau ein Prozess ([ADR 003](003-claude-anbindung.md)); die Nebenfrage braucht dessen Verlauf, darf ihn aber nicht verlängern.

## Optionen

- **(a) Frage in den Hauptprozess schicken und aus dem Verlauf streichen** — verworfen: der Agent wäre blockiert, und der Verlauf der Kommandozeile behielte die Frage.
- **(b) Einmal-Aufruf ohne Verlauf** — verworfen: die Frage braucht den Kontext der Session.
- **(c) Einmal-Aufruf auf einem Fork der Session** — gewählt: `claude -p --resume <Session-ID> --fork-session --no-session-persistence --output-format json` im Arbeitsordner der Session.

## Entscheidung

- **Gleiche Werkzeugliste wie der Hauptprozess** (`common_args` in `agents/claude/process.rs`: Modell, Denkaufwand, Browser-Schalter, Systemprompt-Zusatz, `--allowedTools`, `--add-dir`, `--mcp-config`, Umgebungsvariablen); Werkzeuge sind nur durch `--permission-mode dontAsk` gesperrt. Gemessen am 2026-10-29 gegen eine laufende Opus-Session (~114k Token Verlauf), je zwei Fragen direkt hintereinander:

  | Variante | 1. Frage `cache_read_input_tokens` | 2. Frage `cache_read_input_tokens` | Kosten 1. / 2. Frage |
  |---|---|---|---|
  | ohne Werkzeuge (`--tools "" --safe-mode --strict-mcp-config`) | 0 | 113 554 | 0,92 $ / 0,036 $ |
  | gleiche Werkzeugliste | 146 336 | 146 336 | 0,045 $ / 0,052 $ |

  Ohne Werkzeuge weicht der Anfang des Prompts vom Hauptprozess ab, die erste Frage nach jeder Pause zahlt den ganzen Verlauf voll. Die im Plan vorab festgelegte Regel („zweite Frage entscheidet, bei Gleichstand ohne Werkzeuge“) hätte das übersehen und wurde nach der Messung verworfen. Claude Code macht es genauso: `/btw` hat keine Werkzeuge und ist billig, solange der Cache der Session warm ist. Das Prüfprogramm `side-probe` mit den Argumenten der App bestätigt es: 205 724 von 207 128 Eingabe-Token aus dem Cache schon bei der ersten Frage, `num_turns` 1, Transkript der Session unverändert, auch während der Hauptprozess gerade einen Zug schrieb.
- **Frage und Zitat stehen im Prompt**, nicht im Systemprompt, mit der Anweisung, nur aus dem Verlauf und ohne Werkzeuge zu antworten.
- **`--no-session-persistence`:** ohne den Schalter hinterlässt jeder Fork eine Transkript-Datei im Projektordner der Kommandozeile (gemessen: eine je Frage); mit ihm keine, der Cache trägt trotzdem.
- **Nur im Speicher:** höchstens 20 Nebenfragen je Session (die 21. verdrängt die älteste), höchstens drei gleichzeitig; nach dem Beenden der App weg.
- **Zeitlimit** 120 s, mit LM Studio 300 s.

## Konsequenzen

- Jede Nebenfrage kostet eigene Token; bei warmem Cache wenig, bei kaltem den ganzen Verlauf. Im Planmodus oder nach einem Wechsel von Modell, Denkaufwand oder Browser-Schalter, der erst beim nächsten Start greift, kann der Anfang abweichen und der Cache verfehlt werden.
- Nur die Betriebsarten Claude und Claude + LM Studio; Autark und Codex lehnen mit „Nebenfragen gibt es in dieser Betriebsart noch nicht.“ ab, bevor ein Prozess startet.
- Die letzte, noch nicht geschriebene Antwort des Agenten fehlt dem Fork (wie bei `/btw`).
- Code: `src-tauri/src/side/`, `agents/claude/side.rs`, `sessions/registry/side.rs`, `commands/side.rs`, `examples/side-probe.rs`.

## Nachtrag 2026-11-03: `dontAsk` sperrt nicht

Oben steht, Werkzeuge seien nur durch `--permission-mode dontAsk` gesperrt. Das stimmt nicht: `dontAsk` lehnt nur ab, was nicht freigegeben ist; was `--allowedTools` freigibt, läuft auch dort (gemessen, siehe [ADR 046](046-mcp-zugang.md)). Eine Nebenfrage kann also freigegebene Werkzeuge aufrufen. Dass sie es nicht tut, sagt nur die Anweisung im Prompt. Die Sperre der Session-Werkzeuge liegt am eigenen Schlüssel der Nebenfrage.
