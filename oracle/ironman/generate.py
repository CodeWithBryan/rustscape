"""Unchanged complete pinned IronmanManager/IronmanMode, JVM test oracle only."""
import argparse,hashlib,importlib.util,itertools,json,os,pathlib,subprocess,shutil,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).parent;RUN=ROOT/'runs/ironman-oracle'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
def requests():
    rows=[]
    def add(actions,**kw):rows.append(dict(actions=actions,**kw))
    modes=[None,0,1,2,3]
    for player,mode,minimum in itertools.product([False,True],modes,modes):
        add([dict(op='owner'),dict(op='icons'),dict(op='capture'),dict(op='set',mode=mode),dict(op='is',alias=True),dict(op='default'),dict(op='check',minimum=minimum,alias=True)],player=player)
    for initial,replacement,fail in itertools.product([1,3,None],modes,[False,True]):
        add([dict(op='capture'),dict(op='set',mode=initial),dict(op='default'),dict(op='is'),dict(op='check',minimum=3,alias=True)],player=True,messageMode=replacement,failMessage=fail)
    add([dict(op='is'),dict(op='check',minimum=None),dict(op='owner'),dict(op='icons')],player=False)
    return rows
def generate(check):
    lock=json.loads(c.read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server';assert c.run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()==lock['server']['commit'];c.run(['git','-C',ref,'diff','--quiet','HEAD','--'])
    RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    pin=json.loads(c.read(ROOT/'oracle/auth/json.lock.json'));jar=RUN/'json-simple-1.1.1.jar'
    cached=ROOT/'runs/legacy-interaction-oracle/json-simple-1.1.1.jar'
    if cached.is_file()and cached.stat().st_size==pin['size']and hashlib.sha256(cached.read_bytes()).hexdigest()==pin['sha256']:shutil.copyfile(cached,jar)
    elif not jar.is_file()or hashlib.sha256(jar.read_bytes()).hexdigest()!=pin['sha256']:
        with urllib.request.urlopen(pin['url'],timeout=60)as response:data=response.read()
        assert len(data)==pin['size']and hashlib.sha256(data).hexdigest()==pin['sha256'];jar.write_bytes(data)
    stub=RUN/'adapters/core/game/node/entity/player/Player.java';stub.parent.mkdir(parents=True,exist_ok=True);stub.write_text('package core.game.node.entity.player;public class Player{public void sendMessage(String text){oracle.Support.message(text);}}',encoding='utf-8')
    sources=[ref/'Server/src/main/core/game/node/entity/player/link'/name for name in ['IronmanManager.java','IronmanMode.java']]
    for p in sources:c.read(p)
    c.read(HERE/'Support.java');c.read(pathlib.Path(__file__));c.read(ROOT/'oracle/login/generate.py')
    cp=os.pathsep.join(map(str,[classes,jar]));c.run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*sources,stub,HERE/'Support.java'])
    cases=requests();path=RUN/'input.jsonl';path.write_text(''.join(json.dumps(v,ensure_ascii=False)+'\n'for v in cases),encoding='utf-8')
    exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
    with path.open('rb')as stream:result=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=stream,capture_output=True,check=True)
    rows=[json.loads(v)for v in result.stdout.splitlines()if v];assert len(rows)==len(cases)and all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,cases))
    data=''.join(json.dumps(v,sort_keys=True,separators=(',',':'),ensure_ascii=False)+'\n'for v in rows).encode('utf-8')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged IronmanManager and IronmanMode, default/null modes, ordinal threshold short circuits, icons, actual player/manager aliases and message callback reentry/failure prefixes. Stub Player only forwards sendMessage to the explicit recording service. Canonical Rust ownership is separately regression tested; full Player/profile/content startup is not established.')
    folder=ROOT/'fixtures/ironman';folder.mkdir(exist_ok=True)
    if check:assert(folder/'oracle.jsonl').read_bytes()==data and json.loads((folder/'provenance.json').read_text(encoding='utf-8'))==provenance
    else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2,sort_keys=True)+'\n',encoding='utf-8')
    print(f'{len(rows)} complete original Ironman scenarios '+('reproduced.'if check else'generated.'))
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
