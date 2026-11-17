"""Execute complete original timer registry and ContentAPI factory wrappers."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/timer-registry-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('timers',ROOT/'oracle/timers/generate.py');timers=importlib.util.module_from_spec(spec);spec.loader.exec_module(timers);compiler=timers.compiler;read,run=compiler.read,compiler.run
def requests():
    cases=[];rng=random.Random(530)
    base=dict(a=dict(id='Timer',interval=3,auto=True,**{'class':'FactoryTimer'}),b=dict(id='timer',interval=7,**{'class':'FactoryChild'}),p=dict(id='persist',interval=5,auto=True,**{'class':'FactoryPersistent'}),nil=None)
    def op(name,**kw):return dict(op=name,**kw)
    def add(actions,**kw):cases.append(dict(registryNative=True,timers=json.loads(json.dumps(kw.pop('timers',base))),actions=actions,**kw))
    for id in ['Timer','timer','TIMER','missing','İΣ','İς','𐐀Σ','𐐨σ','AΣ\uffffA']:
        ts=json.loads(json.dumps(base));ts['a']['id']=id
        add([op('registryRegister',timer='a'),op('registryRegister',timer='b'),op('registryRegister',timer='p'),op('registryGet',id=id),op('registryGet',id=id.lower()),op('getOrStart',id=id),op('getOrStart',id=id.upper()),op('auto'),op('has',id=id),op('save'),op('process'),op('auto')],timers=ts)
    for kind in ['registryGet','registryType','spawnType','getOrStart','getOrStartType']:
        for args in [[],[9],[-2**31,'text',None],['wrong']]:
            for cls in ['RSTimer','PlainTimer','PersistentTimer','FactoryChild']:
                ts=json.loads(json.dumps(base));ts['a']['useArgs']=False
                add([op(kind,id='timer',args=args,**{'class':cls}),op('registryRegister',timer='a'),op('registryRegister',timer='p'),op(kind,id='TIMER',args=args,**{'class':cls}),op('save'),op('auto')],timers=ts)
    for event,label in [('retrieve','a'),('factory','a'),('before','created0'),('register','created0')]:
        for callback in [op('fail'),op('autoAppend',timer='b'),op('autoClear'),op('registryRegister',timer='b'),op('register',timer='p'),op('clear'),op('remove',timer='a')]:
            add([op('registryRegister',timer='a'),op('registryRegister',timer='p'),op('auto'),op('registryGet',id='timer',args=[1]),op('auto'),op('save'),op('process')],scripts=[dict(timer=label,event=event,actions=[callback])])
    for at in [1,2,3,4]:add([op('registryRegister',timer='a'),op('registryRegister',timer='b'),op('registryRegister',timer='p'),op('auto')],logFailAt=at)
    for event in ['retrieve','factory']:
        add([op('registryRegister',timer='a'),op('registryType',args=[1]if event=='factory'else[],**{'class':'PlainTimer'}),op('auto'),op('save')],scripts=[dict(timer='a',event=event,actions=[op('registryRegister',timer='p')])])
    for target in ['b','p','nil']:
        ts=json.loads(json.dumps(base));ts['a']['factoryTarget']=target
        add([op('registryRegister',timer='a'),op('registryType',**{'class':'PlainTimer'}),op('spawnType',**{'class':'PlainTimer'}),op('registryGet',id='timer'),op('auto'),op('getOrStartType',**{'class':'PlainTimer'})],timers=ts)
    add([op('registryPut',id='timer',timer='nil'),op('registryGet',id='timer'),op('registryType',**{'class':'RSTimer'}),op('registryRegister',timer='a'),op('autoAppend',timer='nil'),op('auto')])
    add([op('registryRegister',timer='nil'),op('registryRegister',timer='a'),op('auto'),op('auto'),op('removeId',id='Timer'),op('auto'),op('process'),op('auto')])
    for args in [[9],[1.5],['wrong']]:
        ts=json.loads(json.dumps(base));ts['a']['useArgs']=True
        add([op('registryRegister',timer='a'),op('registryGet',id='TIMER',args=args),op('getOrStart',id='timer',args=args)],timers=ts)
    for text in ['{"PERSIST":{"ticksLeft":"9"}}','{"timer":{},"unknown":{}}','{"persist":{"ticksLeft":"bad"}}']:
        add([op('registryRegister',timer='a'),op('registryRegister',timer='p'),op('parse',text=text),op('save'),op('process')])
    for count in [8,9,10,20,40,100]:
        ts={str(i):dict(id=''.join('an'if(i>>bit)&1 else'c0'for bit in range(7)),interval=i,**{'class':'FactoryChild'if i%2 else'FactoryTimer'})for i in range(count)}
        add([op('registryRegister',timer=k)for k in ts]+[op('registryType',**{'class':'RSTimer'}),op('registryType',**{'class':'FactoryChild'})],timers=ts)
    for count in [12,13,24,25,40,100]:
        ts={str(i):dict(id=('Aa'if(i&1)else'BB')+str(i*64),interval=i,**{'class':'FactoryChild'if i%2 else'FactoryTimer'})for i in range(count)}
        add([op('registryRegister',timer=k)for k in ts]+[op('registryType',**{'class':'RSTimer'}),op('registryType',**{'class':'PlainTimer'}),op('registryType',**{'class':'FactoryChild'}),op('auto')],timers=ts)
    for _ in range(80):
        actions=[]
        for _ in range(20):
            name=rng.choice(['registryRegister','registryGet','registryType','spawnType','getOrStart','getOrStartType','auto','process','clear','has','removeId','save'])
            actions.append(op(name,timer=rng.choice(['a','b','p']),id=rng.choice(['Timer','timer','TIMER','persist','missing']),**{'class':rng.choice(['RSTimer','PlainTimer','PersistentTimer','FactoryChild'])},args=rng.choice([[],[1],[-2**31]])))
        add(actions)
    return cases
def generate(check):
    timers.generate(True);RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    jars=compiler.artifacts();runtime=[p for p in jars if p.name.startswith(('kotlin-stdlib-','kotlin-reflect-'))];cp=os.pathsep.join(map(str,[classes,timers.RUN/'classes',timers.RUN/'json-simple-1.1.1.jar',*runtime]));compiler_cp=os.pathsep.join(map(str,jars))
    source=SERVER/'core/game/system/timer/TimerRegistry.kt';read(source);content=read(SERVER/'core/api/ContentAPI.kt')
    signatures=['fun registerTimer (','fun getOrStartTimer (entity: Entity, identifier: String','inline fun <reified T: RSTimer> getOrStartTimer (','fun spawnTimer (identifier: String','inline fun <reified T: RSTimer> spawnTimer (','fun hasTimerActive (entity: Entity, identifier: String','fun getTimer (entity: Entity, identifier: String','fun removeTimer (entity: Entity, identifier: String','inline fun <reified T: RSTimer> hasTimerActive (','inline fun <reified T: RSTimer> getTimer (','inline fun <reified T: RSTimer> removeTimer (']
    api=RUN/'API.kt';api.write_text('package core.api\nimport core.game.node.entity.Entity\nimport core.game.system.timer.*\nfun getWorldTicks()=oracle.Model.ticks()\nfun log(owner:Any,level:Int,message:String)=oracle.RegistryModel.log(level,message)\n'+'\n'.join(compiler.method(content,s)for s in signatures),encoding='utf-8')
    log=RUN/'Log.kt';log.write_text('package core.tools\nobject Log{val ERR=1;val WARN=2}',encoding='utf-8')
    read(HERE/'Support.kt');read(HERE/'RegistryReset.java')
    originals=[SERVER/'core/game/system/timer'/name for name in ['TimerManager.kt','RSTimer.kt','PersistTimer.kt','TimerFlag.kt']]
    compile_cp=os.pathsep.join(map(str,[timers.RUN/'classes',timers.RUN/'json-simple-1.1.1.jar',*runtime]))
    run(['java','-Xmx512m','-cp',compiler_cp,'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,source,*originals,api,log,HERE/'Support.kt',HERE/'RegistryReset.java'])
    run(['javac','--release','11','-cp',cp,'-d',classes,HERE/'RegistryReset.java'])
    requests_file=RUN/'input.jsonl';cases=requests();requests_file.write_bytes(''.join(json.dumps(c,sort_keys=True,ensure_ascii=True)+'\n'for c in cases).encode())
    exe=compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if compiler.JAVA_HOME else'java'
    with requests_file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'--add-opens','java.base/java.util=ALL-UNNAMED','-Dfile.encoding=UTF-8','-cp',cp,'oracle.SupportKt'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
    if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
    output=[json.loads(line)for line in p.stdout.splitlines()if line]
    if len(output)!=len(cases):raise RuntimeError('Missing registry cases')
    data=''.join(json.dumps(c,sort_keys=True,ensure_ascii=True,separators=(',',':'))+'\n'for c in output).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=json.loads(read(ROOT/'upstream.lock.json')),sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(data).hexdigest(),scope='Complete unchanged TimerRegistry and timer classes; verbatim ContentAPI timer wrappers, original Kotlin reflection, actual HashMap iteration/ROOT lowercase. Scripted native-content factory boundary and explicit entity/clock/log adapters. Oracle-only reflection resets empty HashMap capacity for fresh-world isolation; no such code enters runtime. Concrete timers, general arbitrary Java argument/object APIs, world entity startup and world scheduler remain pending.')
    folder=ROOT/'fixtures/timer-registry'
    if check:
        if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Timer registry fixtures/provenance differ')
        print(f'{len(output)} original timer registry/factory cases reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(output)} original timer registry cases.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();compiler.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
