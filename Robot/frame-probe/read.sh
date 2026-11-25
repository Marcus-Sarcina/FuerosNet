#!/bin/sh
cd /home/aspiringsavage/Documents/FuerosNet/crates
R=$(cat ../../FuerosNet-output/bench-packages/streamapp.resource)
python3 tools/bench-package.py logs --resource $R --lines 200 2>&1 | python3 -c "
import sys,json,collections
c=collections.Counter(); last=None; other=[]
for l in sys.stdin:
    l=l.strip()
    try: j=json.loads(l)
    except Exception: other.append(l[:160]); continue
    w=j.get('what')
    if w=='stream.report': last=j['got']
    elif w=='stream.datagram': c['datagram stun' if j['stun'] else 'datagram other']+=1
    elif w=='stream.connect': c['connect port %s first %s'%(j.get('port'),j.get('first'))]+=1
    else: other.append(l[:160])
for o in other: print(o)
for k in sorted(c): print(c[k],k)
if last:
    for k in ('engine','peer_connection','socket','socket_inner','socket_worker','preconnect','transport','fetch_outside','beacon','reach','reach_inner','parts','whole_clip','segments','played_seconds'):
        if k in last: print('  ',k,'=',last[k])
"
