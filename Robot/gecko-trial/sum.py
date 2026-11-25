import sys,json,collections
c=collections.Counter(); last=None; other=[]; pages=0
for l in open(sys.argv[1]):
    l=l.strip()
    try: j=json.loads(l)
    except Exception: continue
    w=j.get('what')
    if w=='stream.report': last=j['got']
    elif w=='stream.datagram': c['datagram stun' if j['stun'] else 'datagram other']+=1
    elif w=='stream.connect': c['connect port %s first %s'%(j.get('port'),j.get('first'))]+=1
    elif w=='stream.page': pages+=1
print('pages served',pages)
for k in sorted(c): print(c[k],k)
if last:
    for k in ('engine','secure_context','peer_connection','socket','socket_inner','socket_worker','preconnect','transport','fetch_outside','beacon','reach','reach_inner','parts','parts_error','whole_clip','whole_error','segments','segments_error','played_seconds','report_error'):
        if k in last: print('  ',k,'=',last[k])
else: print('no report')
