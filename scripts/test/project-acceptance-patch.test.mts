import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';

test('unapplied consumer patch preserves completed evidence without hashing live coordinator logs',t=>{
 const available=spawnSync('python3',['--version'],{encoding:'utf8'});if(available.status!==0){t.skip('Python unavailable; Rust artifact isolation contract remains mandatory');return;}
 const patch=readFileSync(new URL('../../docs/patches/money-printer-panel-acceptance.patch',import.meta.url),'utf8').split('\n');
 const start=patch.findIndex(l=>l.startsWith('+def completed_wave_files'));
 assert.ok(start>=0);const code=[];for(let i=start;i<patch.length&&patch[i]!.startsWith('+');i++)code.push(patch[i]!.slice(1));
 const exercise=`from pathlib import Path
import json, hashlib, tempfile
from concurrent.futures import ThreadPoolExecutor
def digest(p): return hashlib.sha256(p.read_bytes()).hexdigest()
${code.join('\n')}
with tempfile.TemporaryDirectory() as temp:
 parent=Path(temp);old=parent/'finished';old.mkdir();current=parent/'current';current.mkdir()
 (old/'manifest.json').write_text(json.dumps(dict(schema_version=1,wave=old.name,status='PASS',end='complete',cleanup='done',suites=[])))
 (old/'proof.txt').write_text('keep')
 live=parent/'check-live';live.mkdir();(live/'stderr.log').write_text('before')
 running=parent/'running';running.mkdir();(running/'manifest.json').write_text(json.dumps(dict(schema_version=1,wave='running',status='RUNNING',suites=[])))
 before=completed_wave_files(parent,current)
 assert len(before)==2, before
 assert all(k.startswith('finished/') for k in before)
 (live/'stderr.log').write_text('appended by coordinator')
 assert completed_wave_files(parent,current)==before
 (old/'proof.txt').write_text('tampered')
 assert completed_wave_files(parent,current)!=before
 print('completed proof retained; live logs excluded; tamper detected')
`;
 const result=spawnSync('python3',['-c',exercise],{encoding:'utf8'});assert.equal(result.status,0,result.stderr);assert.match(result.stdout,/tamper detected/);
});
