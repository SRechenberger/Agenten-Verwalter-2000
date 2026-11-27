// Selbsttest der Nachrichten aus dem Rahmen eines Artefakts: `node src/features/artifacts/artifactBridge.selftest.ts`
// (`pnpm selftest`). Eine Nachricht ohne `verwalter: 1` oder mit anderem Typ oder Inhalt wird verworfen, und der
// Schlüssel der Liste ändert sich mit dem Zeitpunkt des Zustands. Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
import { parseStateRequest, stateFromFrame } from './artifactBridge.ts';
import { listKey } from './artifactListKey.ts';
import type { Artifact } from '@/lib/bindings/Artifact';
import type { ArtifactList } from '@/lib/bindings/ArtifactList';

let failed = 0;

function check(name: string, ok: boolean): void {
  if (!ok) failed += 1;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}`);
}

const valid = { verwalter: 1, type: 'state', data: { t1: 'bestanden', t2: '{"a":1}' } };

check('a state message is accepted', parseStateRequest(valid)?.t1 === 'bestanden');
check(
  'an empty state is accepted',
  Object.keys(parseStateRequest({ ...valid, data: {} }) ?? { x: 1 }).length === 0,
);
check(
  'a message without verwalter 1 is dropped',
  parseStateRequest({ ...valid, verwalter: undefined }) === null,
);
check(
  'a message with another verwalter version is dropped',
  parseStateRequest({ ...valid, verwalter: 2 }) === null,
);
check(
  'a message of another type is dropped',
  parseStateRequest({ ...valid, type: 'open' }) === null,
);
check(
  'a state with a non-string value is dropped',
  parseStateRequest({ ...valid, data: { a: 1 } }) === null,
);
check(
  'a state with a nested object is dropped',
  parseStateRequest({ ...valid, data: { a: { b: 'c' } } }) === null,
);
check('an array as state is dropped', parseStateRequest({ ...valid, data: ['a'] }) === null);
check('a missing state is dropped', parseStateRequest({ verwalter: 1, type: 'state' }) === null);
check('a null state is dropped', parseStateRequest({ ...valid, data: null }) === null);
check(
  'a message that is no object is dropped',
  parseStateRequest('state') === null && parseStateRequest(null) === null,
);

const OWN = {};
check(
  'a message from the own frame is accepted',
  stateFromFrame(OWN, OWN, valid)?.t1 === 'bestanden',
);
check('a message from another window is dropped', stateFromFrame({}, OWN, valid) === null);
check('a message without a source is dropped', stateFromFrame(null, OWN, valid) === null);
check(
  'a message while no frame exists is dropped',
  stateFromFrame(undefined, undefined, valid) === null,
);

const plan: Artifact = {
  file: 'plan.html',
  title: 'Plan',
  kind: 'html',
  size: 1,
  entries: null,
  viewable: true,
  modifiedAt: 1,
  sessionId: null,
  sessionNumber: null,
  sessionName: null,
  lastSessionNumber: null,
  lastSessionName: null,
};
const withState = (stateModifiedAt?: number): ArtifactList => ({
  dir: '/v/.artefakte',
  baseUrl: 'http://127.0.0.1:1/t/',
  items: [stateModifiedAt === undefined ? plan : { ...plan, stateModifiedAt }],
});
check(
  'the list key changes when only the state time changes',
  listKey(withState()) !== listKey(withState(5)) &&
    listKey(withState(5)) !== listKey(withState(6)) &&
    listKey(withState(5)) === listKey(withState(5)),
);

if (failed > 0) {
  throw new Error(`${String(failed)} Fälle gescheitert`);
}
