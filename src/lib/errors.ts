import type { CommandError } from '@/lib/bindings/CommandError';

export function isCommandError(reason: unknown): reason is CommandError {
  return typeof reason === 'object' && reason !== null && 'kind' in reason;
}

/** Der Anzeige-Text eines Core-Fehlers. `message` ist der Inhalt der Variante (bei `repositoryMissing` nur der Pfad),
 *  deshalb setzt sich der Text je `kind` zusammen. */
export function commandErrorText(reason: unknown): string {
  if (!isCommandError(reason)) {
    return String(reason);
  }
  switch (reason.kind) {
    case 'gitNotFound':
      return 'Git nicht gefunden.';
    case 'repositoryMissing':
      return `Repository nicht gefunden: ${reason.message}`;
    case 'folderNotAllowed':
      return `Diesen Ordner bekommt der Agent nicht: ${reason.message}. Gesperrt sind Laufwerke, der Benutzerordner und alles darüber sowie der Datenordner der App — wähle einen Unterordner.`;
    case 'sessionNotFound':
      return 'Session nicht gefunden.';
    case 'attachmentsWhileWaiting':
      return 'Anhänge gehen erst, wenn die Rückfrage beantwortet ist.';
    case 'voiceModelMissing':
      return 'Sprachmodell fehlt.';
    case 'voiceBusy':
      return 'Es läuft schon ein Diktat.';
    case 'microphone':
      return `Mikrofon nicht verfügbar: ${reason.message.replace(/\.$/, '')}. Prüfe in Windows unter Einstellungen → Datenschutz und Sicherheit → Mikrofon, ob Desktop-Apps zugreifen dürfen.`;
    case 'noAudio':
      return 'Kein Ton vom Mikrofon — ist das richtige Eingabegerät in Windows eingestellt?';
    case 'noSpeech':
      return 'Keine Sprache erkannt.';
    case 'fileNotFound':
      return `Datei nicht gefunden: ${reason.message}`;
    case 'fileNotAllowed':
      return reason.message;
    case 'controlledBy':
      return `Gesteuert von ${reason.message}.`;
    case 'hostUnreachable':
      return `${reason.message} nicht erreichbar.`;
    default:
      return 'message' in reason ? reason.message : reason.kind;
  }
}
