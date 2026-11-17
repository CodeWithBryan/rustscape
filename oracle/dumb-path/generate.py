"""Unchanged DumbPathfinder with pinned original reach/buffer geometry services."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/dumb-path-oracle';SERVER=ROOT/'reference/server/Server/src/main'
def module(name,path):
    spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
base=module('base',ROOT/'oracle/pathfinding/generate.py');c=module('compiler',ROOT/'oracle/login/generate.py');read,run=c.read,c.run
def requests():
    cases=[];rng=random.Random(53041)
    def find(**kw):return dict(op='find',start=[3207,3216,0],end=[3211,3219,0],**kw)
    def add(actions=None,**kw):cases.append(dict(actions=actions or[find(),find(endOffset=[0,0])],**kw))
    for mover in [0,1,2,3,4]:
        for size in [0,1,4,15,26]:
            for dx,dy in [(size,0),(-size,0),(0,size),(0,-size),(size,size),(-size,size),(size,-size),(-size,-size)]:
                for default in [0,0x12c01ff]:add([dict(op='find',start=[3207,3216,0],end=[3207+dx,3216+dy,0],mover=mover)],default=default)
    for mover in [1,2,3,4]:
        for kind in [0,1,3,6,7,8,9,10]:
            for rotation in range(4):add([find(mover=mover,kind=kind,rotation=rotation,size=[2,3],walkingFlag=rotation)],flags=[[3209,y,0x12c01ff]for y in range(3211,3221)])
    for absent in [False,True]:
        for mover in [1,2,3]:
            for destination in [[3207,3216,0],[3208,3216,0],[3212,3220,0]]:add([dict(op='find',start=[3207,3216,0],end=destination,mover=mover,supplier=not absent)])
    for fail in [1,2,5,20]:
        for mover in [1,2,3]:add([find(mover=mover),find()],failAt=fail)
    for mover in [1,2,3]:
        for change in [dict(x=3210,y=3218),dict(found=False),dict(z=2)]:add([find(mover=mover)],hooks=[dict(at=1,actions=[dict(op='fields',values=change)])])
    for field in ['start','end']:
        add([find()],hooks=[dict(at=2,actions=[dict(op='location',field=field,value=[3208,3217,1])])])
    for mover in [1,2,3]:add([find(mover=mover)],hooks=[dict(at=1,actions=[dict(op='find',start=[3209,3217,0],end=[3211,3219,0],mover=mover)])])
    for start,end in [(None,[3208,3216,0]),([3207,3216,0],None)]:add([dict(op='find',start=start,end=end)])
    for _ in range(64):
        flags=[[3207+rng.randrange(-6,7),3216+rng.randrange(-6,7),rng.choice([0x12c01ff,0x80,0x20,0x8,0x2])]for _ in range(24)]
        add([dict(op='find',start=[3207,3216,0],end=[3207+rng.randrange(-7,8),3216+rng.randrange(-7,8),0],mover=rng.randrange(1,5),size=[rng.randrange(3),rng.randrange(3)],kind=rng.choice([0,0,1,9,10]),rotation=rng.randrange(4),near=bool(rng.randrange(2)))],flags=flags)
    return cases
def generate(check):
    # Rebuild/check the shared unchanged original geometry classes before linking
    # this oracle. Its helpers remain Java test artifacts, never runtime code.
    base.c.JAVA_HOME=c.JAVA_HOME;base.generate(True)
    lock=json.loads(read(ROOT/'upstream.lock.json'));c.SOURCES.update(base.c.SOURCES)
    RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=base.RUN/'json-simple-1.1.1.jar'
    cp=os.pathsep.join(map(str,[classes,base.RUN/'classes',std,jar]))
    source=SERVER/'core/game/world/map/path/DumbPathfinder.java';read(source);read(HERE/'Support.java')
    run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,source,HERE/'Support.java'])
    cases=requests();p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
    with p.open('rb')as f:result=subprocess.run(list(map(str,[exe,'-Xmx1g','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
    if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
    rows=[json.loads(l)for l in result.stdout.splitlines()if l];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'tools/port_dumb.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged DumbPathfinder with original Path/Point/Direction and original five reach helpers. Explicit sparse supplier/global clipping, mutable packed locations and reflected field/reentrant callback controls. Compares success/near/ordered points, retained singleton fields, ordered clipping queries, nested results and failure prefixes. Path.walk and NPC/scene/world content binding remain separate; malformed native iterations bounded to65536.')
    folder=ROOT/'fixtures/dumb-path'
    if check:
        if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Dumb-path fixtures/provenance differ')
        print(f'{len(rows)} original dumb-path scenarios reproduced.')
    else:folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} original dumb-path scenarios.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
