#!/usr/bin/env python3
"""seed.py <worktree> <home> <role>: DB im Stand vor dem Update (Migrationen bis 039, ohne 040).
guest: Vorhaben ohne Gruppe, zwei Gruppen, ein Vorhaben mit 6 Sessions (+1 archivierte), ein archiviertes Vorhaben.
host: zwei freigegebene Vorhaben, Fernsteuerung an auf Port 1476. Schreibt ids.json und alte-reihenfolge.json."""
import glob, json, os, sqlite3, sys, time, uuid
worktree, home, role = sys.argv[1:4]
os.makedirs(f'{home}/.verwalter', exist_ok=True); os.makedirs(f'{home}/.claude', exist_ok=True)
con = sqlite3.connect(f'{home}/.verwalter/verwalter.db')
files = [f for f in sorted(glob.glob(f'{worktree}/src-tauri/src/db/migrations/*.sql')) if int(os.path.basename(f)[:3]) <= 39]
for f in files: con.executescript(open(f).read())
con.execute(f'PRAGMA user_version = {len(files)}')
S = lambda k, v: con.execute("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)", (k, v))
S('color_scheme', 'light')
H = 3_600_000
now = time.time() * 1000
ids = {}
def group(key, name, age):
    gid = str(uuid.uuid4()); ids[key] = gid
    con.execute('INSERT INTO project_groups (id, name, created_at) VALUES (?, ?, ?)', (gid, name, now - age * H))
    return gid
def project(name, age, acts, gid=None, archived=None, **extra):
    """acts: Liste (Nummer, Name, Aktivität vor h, archiviert?)"""
    pid = str(uuid.uuid4()); ids[name] = pid
    ws = f'{home}/.verwalter/workspaces/{pid[:8]}'; os.makedirs(ws, exist_ok=True)
    con.execute('INSERT INTO projects (id, name, created_at, group_id, archived_at) VALUES (?, ?, ?, ?, ?)',
                (pid, name, now - age * H, gid, None if archived is None else now - archived * H))
    for col, val in extra.items(): con.execute(f'UPDATE projects SET {col} = ? WHERE id = ?', (val, pid))
    for n, sname, act, arch in acts:
        sid = str(uuid.uuid4()); ids[f'{name}#{n}'] = sid
        con.execute("INSERT INTO sessions (id, name, status, model, effort, mode, created_at, running_ms, context_used, "
                    "context_window, has_agent_history, workspace_dir, project_id, number, last_activity_at, seen_at, archived_at) "
                    "VALUES (?, ?, 'completed', 'haiku', 'low', 'auto', ?, 0, 0, 200000, 0, ?, ?, ?, ?, ?, ?)",
                    (sid, sname, now - age * H + n, ws, pid, n, now - act * H, now, None if not arch else now - 0.1 * H))
if role == 'guest':
    g1 = group('Gruppe Eins', 'Gruppe Eins', 20); g2 = group('Gruppe Zwei', 'Gruppe Zwei', 19)
    project('Alpha', 10, [(1, 'Alpha eins', 5, False)])
    project('Beta', 9, [(1, 'Beta eins', 1, False)])
    project('Gamma', 2, [])
    # Delta: 6 Sessions, Aktivität nicht nach Nummer; #7 archiviert und jünger als alles (zählt nicht).
    project('Delta', 8, [(1, 'D eins', 3.2, False), (2, 'D zwei', 3.0, False), (3, 'D drei', 7.0, False),
                         (4, 'D vier', 6.0, False), (5, 'D fünf', 7.5, False), (6, 'D sechs', 3.1, False), (7, 'D sieben', 0.05, True)])
    project('Echo', 7, [(1, 'Echo eins', 4, False)], g1)
    project('Foxtrot', 6, [], g1)
    project('Golf', 5, [(1, 'Golf eins', 0.5, False)], g1)
    project('Hotel', 4, [(1, 'Hotel eins', 3.5, False)], g2)
    project('India', 11, [(1, 'India eins', 2.5, False)], g2)
    project('Juliett', 30, [(1, 'Juliett eins', 25, True)], None, archived=24)
    open(f'{home}/antwort.md', 'w').write('Antwort der Attrappe.\n')
else:
    project('Host-Vorhaben', 3, [(1, 'Host eins', 1, False)], remote_shared=1)
    project('Host-Zwei', 2, [(1, 'Host zwei', 2, False)], remote_shared=1)
    S('remote_enabled', 'true'); S('remote_port', '1476')
con.commit()
# Alte Reihenfolge (Sidebar.tsx auf dev vor dem Update): projectActivity = max(createdAt, lastActivityAt aktiver Sessions), absteigend, dann createdAt absteigend.
rows = con.execute('''SELECT p.name, p.created_at, (SELECT MAX(last_activity_at) FROM sessions s WHERE s.project_id = p.id AND s.archived_at IS NULL),
                      (SELECT name FROM project_groups g WHERE g.id = p.group_id) FROM projects p WHERE p.archived_at IS NULL''').fetchall()
ranked = sorted(rows, key=lambda r: (-max(r[1], r[2] or r[1]), -r[1]))
old = {}
for name, _, _, g in ranked: old.setdefault(g or '(ohne Gruppe)', []).append(name)
json.dump(ids, open(f'{home}/ids.json', 'w'), indent=1)
json.dump(old, open(f'{home}/alte-reihenfolge.json', 'w'), indent=1, ensure_ascii=False)
print(f'seeded {role}: {len(files)} migrations; alt: {json.dumps(old, ensure_ascii=False)}')
