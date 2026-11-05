// Probe: wie laufen thread/compact/start und review/start im codex app-server ab?
// Aufruf: node codex-probe.cjs <codex.exe> <cwd> <logfile>
const { spawn } = require('child_process');
const fs = require('fs');
const readline = require('readline');

const [exe, cwd, logFile] = process.argv.slice(2);
const log = fs.createWriteStream(logFile);
const child = spawn(exe, ['app-server'], { cwd, stdio: ['pipe', 'pipe', 'pipe'] });
child.stderr.on('data', () => {});
let nextId = 1;
const pending = new Map();
const waiters = [];

function write(obj) {
  child.stdin.write(JSON.stringify(obj) + '\n');
}
function call(method, params) {
  const id = nextId++;
  write({ id, method, params });
  return new Promise((resolve) => pending.set(id, resolve));
}
function waitFor(pred, ms) {
  return new Promise((resolve) => {
    const w = { pred, resolve };
    waiters.push(w);
    setTimeout(() => resolve(null), ms);
  });
}
function short(v) {
  const s = JSON.stringify(v);
  return s.length > 400 ? s.slice(0, 400) + '…' : s;
}

readline.createInterface({ input: child.stdout }).on('line', (line) => {
  let msg;
  try { msg = JSON.parse(line); } catch { return; }
  if (msg.id !== undefined && pending.has(msg.id) && !msg.method) {
    log.write(`RESP ${msg.id} ${short(msg.result ?? msg.error)}\n`);
    pending.get(msg.id)(msg);
    pending.delete(msg.id);
    return;
  }
  if (msg.method && msg.id !== undefined) {
    log.write(`REQ-FROM-CODEX ${msg.method}\n`);
    write({ id: msg.id, result: { decision: 'decline' } });
    return;
  }
  if (msg.method) {
    if (/delta$|Delta$/.test(msg.method)) return;
    const p = msg.params || {};
    const item = p.item ? ` item.type=${p.item.type} ${short(p.item)}` : '';
    const turn = p.turn ? ` turn.id=${p.turn.id} status=${p.turn.status}` : '';
    log.write(`NOTE ${msg.method}${turn}${item}${!item && !turn ? ' ' + short(p) : ''}\n`);
    for (const w of [...waiters]) {
      if (w.pred(msg)) { waiters.splice(waiters.indexOf(w), 1); w.resolve(msg); }
    }
  }
});

(async () => {
  await call('initialize', { clientInfo: { name: 'probe', version: '0' } });
  write({ method: 'initialized' });
  const started = await call('thread/start', { cwd, ephemeral: true, approvalPolicy: 'never', sandbox: 'read-only' });
  const threadId = started.result.thread.id;
  log.write(`--- turn/start\n`);
  await call('turn/start', { threadId, input: [{ type: 'text', text: 'Antworte nur mit dem Wort OK.' }] });
  await waitFor((m) => m.method === 'turn/completed', 120000);
  log.write(`--- thread/compact/start\n`);
  await call('thread/compact/start', { threadId });
  await waitFor((m) => m.method === 'turn/completed', 120000);
  await new Promise((r) => setTimeout(r, 3000));
  log.write(`--- review/start custom\n`);
  await call('review/start', { threadId, delivery: 'inline', target: { type: 'custom', instructions: 'Es gibt nichts zu prüfen. Antworte nur: Keine Befunde.' } });
  await waitFor((m) => m.method === 'turn/completed', 180000);
  await new Promise((r) => setTimeout(r, 2000));
  log.write(`--- Ende\n`);
  log.end();
  child.stdin.end();
  setTimeout(() => process.exit(0), 3000);
})();
