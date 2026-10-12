#!/usr/bin/env python3
"""Read-only bounded ZIP audit; persist redacted, de-duplicated factual evidence."""
import argparse, hashlib, json, zipfile
from pathlib import Path
p = argparse.ArgumentParser(); p.add_argument('reports', type=Path); p.add_argument('output', type=Path); a = p.parse_args()
logs, archives = {}, []
for path in sorted(a.reports.glob('report-*.zip')):
    entry = {'name': path.name, 'sha256': hashlib.file_digest(path.open('rb'), 'sha256').hexdigest(), 'logs': []}
    with zipfile.ZipFile(path) as z:
        assert sum(i.file_size for i in z.infolist()) < 100 * 1024 * 1024
        for i in z.infolist():
            if not i.filename.endswith('.jsonl'): continue
            data = z.read(i); key = hashlib.sha256(data).hexdigest(); entry['logs'].append(key)
            if key in logs: continue
            events = [json.loads(line) for line in data.splitlines()]
            picked = []
            for e in events:
                category, event, fields = e.get('category'), e.get('event'), e.get('fields', {})
                if category in ('filepicker', 'import', 'launcher') or event in ('return_ok', 'operation_error'):
                    picked.append({k: e[k] for k in ('utc','category','event','fields','session') if k in e})
            logs[key] = {'records': len(events), 'process': events[0].get('process'), 'session': events[0].get('session'), 'timeline': picked}
    archives.append(entry)
a.output.parent.mkdir(parents=True, exist_ok=True)
a.output.write_text(json.dumps({'archives': archives, 'unique_logs': logs}, indent=2) + '\n')
print(json.dumps({'archives': len(archives), 'unique_logs': len(logs), 'tack_sessions': len({v['session'] for v in logs.values() if v['process'] == 'tack'})}))
