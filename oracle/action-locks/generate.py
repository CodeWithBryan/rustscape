"""Complete unchanged ActionLocks, Lock and LockElapse with an explicit world clock."""
import argparse
import hashlib
import importlib.util
import itertools
import json
import os
import pathlib
import random
import subprocess
import urllib.request

ROOT=pathlib.Path(__file__).resolve().parents[2]
HERE=pathlib.Path(__file__).parent
RUN=ROOT/'runs/action-locks-oracle'
SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py')
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
read,run=c.read,c.run

def op(operation,**kwargs):return dict(op=operation,**kwargs)
def requests():
    rows=[]
    def add(actions,**kwargs):rows.append(dict(actions=actions,**kwargs))
    times=[-2147483648,-1,0,1,2147483647]
    lengths=[-2147483648,-1,0,1,4,2147483647]
    for time,ticks,kind in itertools.product(times,lengths,['all','movement','teleport','component','interaction','equipment']):
        add([op('new',name='e',message=[65,0,0xd800]),op('equipment',name='e'),op('lock',kind=kind,ticks=ticks),op('locked',kind=kind),op('tick',value=time+1 if time<2147483647 else -2147483648),op('locked',kind=kind),op('lock',kind=kind,ticks=-1),op('unlock',kind=kind),op('locked',kind=kind)],tick=time)
    for time in times:
        add([op('lock',kind='all',forever=True),op('lockref',name='m',forever=True),op('lock',kind='teleport',ticks=12),op('lock',kind='component',ticks=14),op('unlock',kind='all'),op('equipment',name='m'),op('unlockref',name='m'),op('equipment',name=None),op('locked',kind='equipment')],tick=time)
    for samples in [[0,1,2,3,4],[2147483647,-2147483648,-1,0],[10,3,100,8,11]]:
        add([op('lock',kind='all',ticks=5),op('locked',kind='all'),op('lockref',name='m',forever=True),op('lockedref',name='m')],samples=samples)
    for key in [None,[],[97],[0],[0xd800],[0xd83d,0xde00]]:
        add([op('customLocked',key=key),op('custom',key=key,ticks=5,event='f0',name='old'),op('custom',key=key,ticks=1,name='alias'),op('same',name='old',other='alias'),op('custom',key=key,ticks=9,event='f0'),op('remove',key=key,cache=False,node=None),op('customLocked',key=key),op('remove',key=key),op('custom',key=key,ticks=2,event='f1',name='new'),op('same',name='old',other='new')])
    callbacks=[
        [op('lockref',name='passed',ticks=12)],
        [op('custom',key=[97],ticks=11,event='f1')],
        [op('remove',key=[97],cache=True,event=None),op('custom',key=[97],ticks=11,event='f1',name='replacement')],
        [op('elapse',name='passed',event='f1'),op('message',name='passed',value=[88])],
        [op('custom',key=[97],ticks=11,event=None),op('fail')],
        [op('elapse',name='passed',event=None),op('remove',key=[97]),op('custom',key=[97],ticks=11,event='f1')],
    ]
    # Avoid recursively invoking the same unchanged f0 in the remove/recreate case.
    callbacks[2].insert(0,op('elapse',name='passed',event=None))
    for body,cache in itertools.product(callbacks,[False,True]):
        add([op('custom',key=[97],ticks=5,event='f0',name='old'),op('remove',key=[97],cache=cache,node=7),op('customLocked',key=[97]),op('remove',key=[97]),op('getMessage',name='old')],callbacks={'f0':body,'f1':[op('message',name='passed',value=[89])]})
    for nth in range(1,7):
        add([op('lock',kind='all',ticks=5),op('lock',kind='all',forever=True),op('custom',key=[97],ticks=5,event='f0'),op('remove',key=[97])],failClockAt=nth)
    for nth,body in itertools.product([1,2,3,4],[[op('unlockref',name='m')],[op('tick',value=100)],[op('lockref',name='m',ticks=50)],[op('fail')]]):
        add([op('lock',kind='all',ticks=10),op('lockedref',name='m'),op('lockref',name='m',ticks=5)],hooks={str(nth):body},tick=7)
    add([op('new',name='x',message=[]),op('elapse',name='x',event='f0'),op('message',name='x',value=[0xd800,0,0xd83d,0xde00]),op('getMessage',name='x'),op('message',name='x',value=None),op('elapse',name='x',event=None),op('getMessage',name='x')])
    rng=random.Random(530)
    for _ in range(80):
        actions=[op('new',name='e'),op('equipment',name='e')]
        for _ in range(24):
            name=rng.choice(['lock','unlock','locked','custom','remove','customLocked','tick','message','elapse'])
            if name in ['lock','unlock','locked']:actions.append(op(name,kind=rng.choice(['all','movement','teleport','component','interaction','equipment']),ticks=rng.choice(lengths)))
            elif name in ['custom','remove','customLocked']:actions.append(op(name,key=rng.choice([None,[],[97],[98]]),ticks=rng.choice(lengths),event=rng.choice([None,'f0','f1']),cache=rng.choice([True,False])))
            elif name=='tick':actions.append(op(name,value=rng.choice(times)))
            else:actions.append(op(name,name=rng.choice(['m','e','i']),value=rng.choice([None,[],[65],[0xd800]]),event=rng.choice([None,'f0','f1'])))
        add(actions,tick=rng.choice(times),callbacks={'f0':[op('message',name='passed',value=[65])],'f1':[op('elapse',name='passed',event=None)]})
    return rows

def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json'))
    for part in ['server','client']:
        path=ROOT/'reference'/part
        assert run(['git','-C',path,'rev-parse','HEAD']).stdout.decode().strip()==lock[part]['commit']
        run(['git','-C',path,'diff','--quiet','HEAD','--'])
    RUN.mkdir(exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'));jar=RUN/'json-simple-1.1.1.jar'
    if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as response:data=response.read()
        assert len(data)==artifact['size']and hashlib.sha256(data).hexdigest()==artifact['sha256'];jar.write_bytes(data)
    java=[]
    def adapter(name,text):
        path=RUN/'adapters'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf-8');java.append(path)
    adapter('core/game/world/GameWorld.java','package core.game.world;public class GameWorld{public static int getTicks(){return oracle.Support.ticks();}}')
    adapter('core/game/node/Node.java','package core.game.node;public class Node{public final int id;public Node(int id){this.id=id;}}')
    for name in ['ActionLocks.java','Lock.java','LockElapse.java']:
        path=SERVER/'core/game/node/entity/lock'/name;read(path);java.append(path)
    read(HERE/'Support.java');java.append(HERE/'Support.java')
    cp=os.pathsep.join(map(str,[classes,jar]));run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
    cases=requests();exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
    result=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),input=''.join(json.dumps(row,sort_keys=True)+'\n'for row in cases).encode(),capture_output=True,cwd=ROOT,timeout=60)
    if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
    rows=[json.loads(line)for line in result.stdout.splitlines()if line];assert len(rows)==len(cases)
    data=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    initial=json.loads(read(HERE/'initial.lock.json'));assert hashlib.sha256(b''.join(data.splitlines(keepends=True)[:initial['caseCount']])).hexdigest()==initial['sha256']
    provenance=dict(format=1,initialBaseline=initial,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged ActionLocks.java, Lock.java and LockElapse.java; explicit GameWorld clock samples/failures/reentry and native callback recording. Reflection observes private fields and aliases without changing reference source. No gameplay relies on diagnostic IDs, map inspection order or recording executors. Runtime ownership, actor integration, native elapse and world admission are separately regression-tested.')
    folder=ROOT/'fixtures/action-locks'
    if check:
        assert(folder/'oracle.jsonl').read_bytes()==data
        assert json.loads((folder/'provenance.json').read_text(encoding='utf-8'))==provenance
        print(f'{len(rows)} complete original action-lock scenarios reproduced.')
    else:
        folder.mkdir(exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} complete original action-lock scenarios.')
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true');parser.add_argument('--java-home',type=pathlib.Path);args=parser.parse_args();c.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(args.check)
