// Kurzer Name des Agenten einer Betriebsart für Kasten und Arbeitsanzeige. Importiert nur Typen, damit
// `node src/features/chat/sessionJump.selftest.ts` die Datei laden kann (`pnpm selftest`).
import type { OperatingMode } from '@/lib/bindings/OperatingMode';

const AGENT_NAMES: Readonly<Record<OperatingMode, string>> = {
  claude: 'Claude',
  claudeCodeLocal: 'Claude (LM Studio)',
  standalone: 'Autark',
  codex: 'Codex',
};

export function agentName(mode: OperatingMode): string {
  return AGENT_NAMES[mode];
}
