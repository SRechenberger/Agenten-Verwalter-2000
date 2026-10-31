import { invoke } from '@tauri-apps/api/core';
import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
import type { VoiceLevelEvent } from '@/lib/bindings/VoiceLevelEvent';
import type { VoiceModelEvent } from '@/lib/bindings/VoiceModelEvent';
import type { VoiceModelState } from '@/lib/bindings/VoiceModelState';
import type { VoicePartialEvent } from '@/lib/bindings/VoicePartialEvent';

const VOICE_MODEL_EVENT = 'voice://model';
const VOICE_LEVEL_EVENT = 'voice://level';
const VOICE_PARTIAL_EVENT = 'voice://partial';

/** Zustand des Sprachmodells: fehlt, wird geladen (mit Bytes) oder ist fertig.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `io` */
export function loadVoiceModelState(): Promise<VoiceModelState> {
  return invoke<VoiceModelState>('voice_model_status');
}

/** Startet den Download des Sprachmodells im Hintergrund; läuft schon einer, passiert nichts. Fortschritt
 *  und Ende kommen über `onVoiceModel`.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `io`, `voiceDownload` */
export async function downloadVoiceModel(): Promise<void> {
  await invoke('voice_model_download');
}

/** Bricht einen laufenden Download ab; ohne Download passiert nichts. */
export async function cancelVoiceModelDownload(): Promise<void> {
  await invoke('voice_model_cancel_download');
}

/** Meldet jeden Zustandswechsel des Sprachmodells, beim Laden höchstens alle 250 ms. */
export function onVoiceModel(callback: (event: VoiceModelEvent) => void): Promise<UnlistenFn> {
  return listen<VoiceModelEvent>(VOICE_MODEL_EVENT, (event: Event<VoiceModelEvent>) => {
    callback(event.payload);
  });
}

/** Startet ein Diktat vom Standard-Mikrofon; kehrt zurück, sobald die Aufnahme läuft.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `voiceModelMissing`, `voiceBusy`, `microphone` */
export async function startDictation(): Promise<void> {
  await invoke('voice_start');
}

/** Beendet die Aufnahme und liefert den gesamten erkannten Text, sobald auch der letzte Abschnitt erkannt ist.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `voiceCancelled`, `noAudio`, `noSpeech`,
 *  `microphone`, `internal` (auch: Sprachmodell lässt sich nicht laden) */
export function stopDictation(): Promise<string> {
  return invoke<string>('voice_stop');
}

/** Bricht Aufnahme oder Erkennung ab; ohne laufendes Diktat passiert nichts. */
export async function cancelDictation(): Promise<void> {
  await invoke('voice_cancel');
}

/** Pegel der laufenden Aufnahme, etwa alle 50 ms. */
export function onVoiceLevel(callback: (event: VoiceLevelEvent) => void): Promise<UnlistenFn> {
  return listen<VoiceLevelEvent>(VOICE_LEVEL_EVENT, (event: Event<VoiceLevelEvent>) => {
    callback(event.payload);
  });
}

/** Nach jedem erkannten Abschnitt der gesamte bisher erkannte Text des Diktats (ersetzt die Vorschau). */
export function onVoicePartial(callback: (event: VoicePartialEvent) => void): Promise<UnlistenFn> {
  return listen<VoicePartialEvent>(VOICE_PARTIAL_EVENT, (event: Event<VoicePartialEvent>) => {
    callback(event.payload);
  });
}
