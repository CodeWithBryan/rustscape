"""Execute complete original timer classes; world/content/registry are explicit adapters."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/timers-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');compiler=importlib.util.module_from_spec(spec);spec.loader.exec_module(compiler)
read,run=compiler.read,compiler.run

def requests():
    cases=[];rng=random.Random(530)
    def op(name,**args):return dict(op=name,**args)
    timers=dict(a=dict(id='same',interval=3),b=dict(id='same',interval=7,**{'class':'ChildTimer'}),p=dict(id='persist',interval=5,**{'class':'PersistentTimer'}),q=dict(id='persist',interval=-1,**{'class':'ChildPersistent'}),nil=None)
    def add(actions,**kw):cases.append(dict(timers=json.loads(json.dumps(kw.pop('timers',timers))),actions=actions,**kw))
    for player in [True,False]:
        for modal,delay in [(False,0),(False,100),(True,0)]:
            for tick in [-2**31,-1,0,1,2**31-1]:
                ts=json.loads(json.dumps(timers));ts['a']['soft']=True;ts['b']['death']=True;ts['p']['auto']=True
                add([op('register',timer=t)for t in ['a','b','p','q']]+[op('save'),op('process'),op('save'),op('tick',value=8),op('process'),op('getId',id='same'),op('death'),op('save'),op('getClass',**{'class':'RSTimer'}),op('process'),op('clear')],timers=ts,player=player,modal=modal,delay=delay,tick=tick)
    for stage in ['before','register','initial','run','remove','save','parse']:
        callbacks=[op('fail'),op('clear'),op('register',timer='b'),op('remove',timer='p'),op('rawAdd',list='active',timer='b'),op('rawClear',list='pending'),op('rawAdd',list='pending',timer='a'),op('setTimer',timer='p',field='interval',value=-9),op('tick',value=2**31-1)]
        for callback in callbacks:
            actions=[op('register',timer='p'),op('process'),op('tick',value=20),op('process'),op('save'),op('remove',timer='p'),op('process'),op('parse',text='{"persist":{"ticksLeft":"3"}}'),op('save')]
            label='created0'if stage=='parse'else'p'
            add(actions,registry={'persist':'p'},scripts=[dict(timer=label,event=stage,actions=[callback])])
    for result in [True,False]:
        ts=json.loads(json.dumps(timers));ts['p']['result']=result
        add([op('process'),op('getId',id='persist'),op('save'),op('process')],timers=ts,active=['p','q'],tick=0,step=1,scripts=[dict(timer='p',event='remove',actions=[op('fail')])])
    for active,pending in [(['a','b'],[]),(['a'],['b']),([],['a','b']),(['a','a'],['a']),(['nil'],['p']),(['a','nil'],[])]:
        add([op('getId',id='same'),op('getClass',**{'class':'RSTimer'}),op('removeId',id='same'),op('getId',id='same'),op('removeClass',**{'class':'PlainTimer'}),op('death'),op('save'),op('process')],active=active,pending=pending)
    for label in ['a','b']:
        for listname in ['active','pending','removing']:
            add([op('removeClass',**{'class':'PlainTimer'}),op('process')],active=['a','b'],pending=['a'],scripts=[dict(timer=label,event='remove',actions=[op('rawClear',list=listname)])])
    for root in ['{}','{"persist":{"ticksLeft":"12"}}','{"persist":{"ticksLeft":-3},"unknown":{}}','{"persist":null}','{"persist":[]}','{"persist":{}}','{"persist":{"ticksLeft":"2147483648"}}','{"plain":{"ticksLeft":"3"}}','{"persist":{"ticksLeft":true}}','{"persist":{"ticksLeft":"१२"}}','{"persist":{"ticksLeft":"+1"},"other":{"ticksLeft":"9"}}']:
        add([op('parse',text=root),op('save'),op('process'),op('save')],registry={'persist':'p','other':'q','plain':'a'},tick=123,step=1)
    for tick in [-2**31,-1,0,2**31-1]:
        for next in [-2**31,-1,0,2**31-1]:
            add([op('setTimer',timer='p',field='next',value=next),op('baseSave',timer='p'),op('baseParse',timer='p',text='{"ticksLeft":"-17"}'),op('save')],active=['p'],tick=tick,step=1)
    for name in ['PlainTimer','PersistentTimer','ChildTimer','ChildPersistent','RSTimer']:
        add([op('getClass',**{'class':name}),op('removeClass',**{'class':name}),op('getClass',**{'class':name}),op('save'),op('process')],active=['a','p'],pending=['b','q'])
    add([op('register',timer='nil'),op('optionalRegister',timer='nil'),op('remove',timer='nil'),op('process')],pending=['nil'])
    add([op('save'),op('process'),op('save')],active=['p'],pending=['q'],removing=['p'],tick=10)
    add([op('save')],active=['p','q'],scripts=[dict(timer='p',event='save',actions=[op('rawAdd',list='active',timer='a')])])
    for _ in range(100):
        actions=[]
        for _ in range(24):
            name=rng.choice(['register','remove','removeId','getId','getClass','removeClass','process','death','save','tick','modal','delay','setTimer','clear'])
            actions.append(op(name,timer=rng.choice(['a','b','p','q']),id=rng.choice(['same','persist','missing']),**{'class':rng.choice(['RSTimer','PlainTimer','PersistentTimer'])},value=rng.choice([-2**31,-3,0,1,10,2**31-1])if name!='modal'else bool(rng.randrange(2)),field=rng.choice(['interval','next','last'])))
        add(actions,player=bool(rng.randrange(2)),step=rng.choice([0,1,2**31-1]))
    return cases

def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json'))
    for part in ['server','client']:
        path=ROOT/'reference'/part
        if run(['git','-C',path,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
        run(['git','-C',path,'diff','--quiet','HEAD','--'])
    RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    jars=compiler.artifacts();artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'));jar=RUN/'json-simple-1.1.1.jar'
    if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as response:data=response.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    runtime=[p for p in jars if p.name.startswith(('kotlin-stdlib-','kotlin-reflect-'))];cp=os.pathsep.join(map(str,[classes,jar,*runtime]))
    inputs=[]
    def adapter(name,text):
        path=RUN/'adapters'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf-8');inputs.append(path)
    adapter('Entity.kt','package core.game.node.entity\nimport core.game.system.timer.TimerManager\nopen class Entity {val timers=TimerManager(this);val scripts=Scripts();val name:String? get()="entity";fun asPlayer()=this as core.game.node.entity.player.Player}\nclass Scripts {val delay:Int get(){oracle.Model.events.add(listOf("delay",oracle.Model.delay));return oracle.Model.delay}}')
    adapter('Player.kt','package core.game.node.entity.player\nclass Player:core.game.node.entity.Entity(){fun hasModalOpen():Boolean {oracle.Model.events.add(listOf("modal",oracle.Model.modal));return oracle.Model.modal}}')
    content=read(SERVER/'core/api/ContentAPI.kt');register=compiler.method(content,'fun registerTimer (entity: Entity, timer: RSTimer?)')
    adapter('API.kt','package core.api\nimport core.game.node.entity.Entity\nimport core.game.system.timer.RSTimer\nfun getWorldTicks()=oracle.Model.ticks()\nfun log(owner:Any,level:Any,message:String){oracle.Model.events.add(listOf("log",message))}\n'+register)
    adapter('Log.kt','package core.tools\nobject Log {val ERR=1}')
    adapter('TimerRegistry.kt','package core.game.system.timer\nobject TimerRegistry {fun getTimerInstance(identifier:String):RSTimer?=oracle.Model.resolve(identifier)}')
    for name in ['TimerManager.kt','RSTimer.kt','PersistTimer.kt','TimerFlag.kt']:
        path=SERVER/'core/game/system/timer'/name;read(path);inputs.append(path)
    support=HERE/'Support.kt';read(support);inputs.append(support)
    compiler_cp=os.pathsep.join(map(str,jars))
    run(['java','-Xmx512m','-cp',compiler_cp,'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*inputs])
    cases=requests();file=RUN/'input.jsonl';file.write_bytes(''.join(json.dumps(c,sort_keys=True)+'\n'for c in cases).encode())
    executable=compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if compiler.JAVA_HOME else'java'
    with file.open('rb')as stdin:result=subprocess.run(list(map(str,[executable,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.SupportKt'])),cwd=ROOT,stdin=stdin,capture_output=True,timeout=120)
    if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
    output=[json.loads(line)for line in result.stdout.splitlines()if line]
    if len(output)!=len(cases):raise RuntimeError('Missing timer cases')
    data=''.join(json.dumps(c,sort_keys=True,ensure_ascii=True,separators=(',',':'))+'\n'for c in output).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged TimerManager, RSTimer, PersistTimer and TimerFlag; verbatim nullable ContentAPI registerTimer. Actual JSON-simple parser/HashMap persistence and original Kotlin reflective instance construction. Explicit entity/player/modal/script/clock/log adapters, controlled registry lookup and scripted content subclasses. No full TimerRegistry registration/auto timers, concrete gameplay timers, world scheduler, profile or world entry claim.')
    folder=ROOT/'fixtures/timers'
    if check:
        if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Timer oracle/provenance differs; review before regeneration')
        print(f'{len(output)} original timer cases reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(output)} original timer cases.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();compiler.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
