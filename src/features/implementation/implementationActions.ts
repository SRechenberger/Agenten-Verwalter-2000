// Befehle der Umsetzung, die die Kartei und das Dashboard gemeinsam nutzen.
import { flatStations, type StationRef } from '@/features/implementation/implementationOrder';
import { patchEdit } from '@/features/implementation/implementationGraph';
import {
  proposalAppliedWithFailures,
  stationNotFound,
} from '@/features/implementation/implementationTexts';
import type { ProposalOverride } from '@/features/implementation/PlanChangeCard';
import type { ImplementationView as Implementation } from '@/lib/bindings/ImplementationView';
import type { StationPatch } from '@/lib/bindings/StationPatch';
import { commandErrorText } from '@/lib/errors';
import {
  applyImplementationCancel,
  applyImplementationChange,
  discardImplementationCancel,
  editPlan,
  listImplementations,
  setStationAgent,
} from '@/lib/implementation';

/** Die Stationen einer Umsetzung, wie der Core sie jetzt hält; nach einem Befehl, bevor das Ereignis da ist. */
export async function freshStations(implementationId: string): Promise<StationRef[]> {
  const fresh: Implementation | undefined = (await listImplementations()).find(
    (candidate: Implementation) => candidate.id === implementationId,
  );
  return fresh === undefined ? [] : flatStations(fresh);
}

/** „Übernehmen“ mit geänderten Werten: erst der Vorschlag wie vorgeschlagen (damit die Plan-Änderung mit
 *  ihrer Quelle vermerkt ist), dann Typ, Modell und Denkaufwand je Station und der Auftrag als Delta. Die
 *  Stationen findet die Funktion über ihre Kennung, neue ohne Kennung über Lane und Name unter den neuen. */
export async function applyEditedProposal(
  implementationId: string,
  before: ReadonlySet<string>,
  overrides: readonly ProposalOverride[],
): Promise<void> {
  await applyImplementationChange(implementationId, true);
  const refs: StationRef[] = await freshStations(implementationId);
  const used = new Set<string>();
  const briefs: StationPatch[] = [];
  const failures: string[] = [];
  for (const override of overrides) {
    // Der Core speichert den Namen gekürzt.
    const stage: string = override.stage.trim();
    const target: StationRef | undefined = refs.find(
      (ref: StationRef) =>
        !used.has(ref.station.planId) &&
        (override.planId === null
          ? !before.has(ref.station.planId) &&
            ref.phase.key === override.lane &&
            ref.station.stage === stage
          : ref.station.planId === override.planId),
    );
    if (target === undefined) {
      failures.push(stationNotFound(stage, override.lane));
      continue;
    }
    used.add(target.station.planId);
    const { station } = target;
    if (override.changed.agent || override.changed.model || override.changed.effort) {
      try {
        await setStationAgent(implementationId, station.id, {
          agent: override.agent,
          model: override.model,
          effort: override.effort,
          prompt: station.prompt,
          skill: station.skill,
        });
      } catch (reason: unknown) {
        failures.push(`${stage} (Lane ${override.lane}): ${commandErrorText(reason)}`);
        continue;
      }
    }
    if (override.changed.brief) {
      briefs.push({ id: station.planId, brief: override.brief });
    }
  }
  if (briefs.length > 0) {
    try {
      await editPlan(implementationId, patchEdit(...briefs));
    } catch (reason: unknown) {
      failures.push(commandErrorText(reason));
    }
  }
  if (failures.length > 0) {
    throw new Error(proposalAppliedWithFailures(failures));
  }
}

/** „Bestätigen“ (`true`) oder „Verwerfen“ der Anfrage „Umsetzung abbrechen?“ (ADR 066). Gibt den Satz zurück, wenn
 *  die Umsetzung abgebrochen, ihr Ordner aber nicht gelöscht wurde; sonst `null`. */
export async function answerCancelRequest(
  implementationId: string,
  accept: boolean,
): Promise<string | null> {
  if (!accept) {
    await discardImplementationCancel(implementationId);
    return null;
  }
  return applyImplementationCancel(implementationId);
}
