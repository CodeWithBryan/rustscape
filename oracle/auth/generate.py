"""Compile complete pinned account/authentication classes with explicit clock/entropy adapters."""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import subprocess
import urllib.request

ROOT=pathlib.Path(__file__).resolve().parents[2]
HERE=pathlib.Path(__file__).resolve().parent
RUN=ROOT/'runs/auth-oracle'
SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('login_compiler',ROOT/'oracle/login/generate.py')
compiler=importlib.util.module_from_spec(spec);spec.loader.exec_module(compiler)
read=compiler.read
run=compiler.run

def requests():
    cases=[]
    def add(actions,mode='development',admin=True,now=1700000000123):
        cases.append(dict(kind='sequence',mode=mode,admin=admin,now=now,actions=actions))
    def new(id='a',fields=None):return dict(op='new',id=id,fields=fields or {})
    def set_(field,value,id='a'):return dict(op='set',id=id,field=field,value=value)
    for now in [-2**63,-1001,-1,0,1,999,1000,1700000000123,2**63-1]:
        add([new(),dict(op='timestamp_nanos',id='a',value=123456789),dict(op='timestamp_time',id='a',value=-1),
             set_(17,dict(millis=now,nanos=999999999)),dict(op='baseline',id='a'),
             dict(op='timestamp_time',id='a',value=1234),dict(op='uninitialize',id='a'),set_(3,2)],now=now)
    for index in range(18):
        if index in [2,3,4]:values=[-2**31,2**31-1,1]
        elif index in [7,8,14,15]:values=[-2**63,2**63-1,1]
        elif index==16:values=[True]
        elif index==17:values=[dict(millis=1,nanos=17)]
        else:values=['value','A😀Ω\x80','']
        for value in values:add([new(),set_(index,value),dict(op='baseline',id='a'),set_(index,value)])
    add([dict(op='taken',key='Alice'),dict(op='get',id='a',key='Alice'),dict(op='get',id='b',key='Alice'),
         dict(op='store',id='a'),dict(op='get',id='blank',key=''),set_(0,'renamed'),dict(op='update',id='a'),
         dict(op='get',id='renamed',key='renamed'),dict(op='remove',id='a'),dict(op='taken',key='Alice'),
         new('one',{'0':'Aa'}),dict(op='store',id='one'),new('two',{'0':'BB'}),dict(op='store',id='two'),
         dict(op='get',id='missing',key='😀'),set_(16,True,'one'),set_(5,'127.0.0.1','one'),
         dict(op='friends',key='Aa'),dict(op='ip',key='127.0.0.1')])
    for admin in [False,True]:
        for name in ['alice','Alice','RUST_CLIENT','Aa','BB','']:
            add([dict(op='login',id='first',username=name,password='ignored'),dict(op='login',id='second',username=name,password='different'),
                 dict(op='can_create',id='first'),dict(op='update_password',username=name,password='changed'),
                 dict(op='password',stored='changed',password='changed'),dict(op='password',stored='changed',password='wrong')],admin=admin)
    for mode in ['development','production']:
        add([new(fields={'0':'Alice','1':'password'}),dict(op='clock',value=1700000000999),dict(op='create',id='a'),
             dict(op='login',id='login',username='Alice',password='password'),dict(op='update_password',username='Alice',password='next')],mode=mode)
        add([new(fields={'0':'alice','1':'password'}),dict(op='create',id='a'),dict(op='can_create',id='a'),
             dict(op='login',id='first',username='Alice',password='password'),dict(op='login',id='wrong',username='alice',password='bad'),
             dict(op='update_password',username='alice',password='next'),dict(op='login',id='next',username='alice',password='next')],mode=mode)
        for fail in ['taken','get','store','update']:
            add([new(fields={'0':'alice','1':'password'}),dict(op='create',id='a'),dict(op='fail_storage',value=fail),
                 dict(op='login',id='login',username='alice',password='password'),dict(op='update_password',username='alice',password='next'),
                 new('b',{'0':'bob','1':'password'}),dict(op='create',id='b'),dict(op='can_create',id='b')],mode=mode)
        add([dict(op='fail_hash',value=True),new(fields={'0':'alice','1':'password'}),dict(op='create',id='a'),
             dict(op='update_password',username='missing',password='new'),dict(op='login',id='login',username='alice',password='password')],mode=mode)
    for ban in [-2**63,1700000000122,1700000000123,1700000000124,2**63-1]:
        for online in [False,True]:
            for password in ['password','wrong']:
                add([new(fields={'0':'alice','1':'password','8':ban,'16':online}),dict(op='create',id='a'),
                     dict(op='login',id='login',username='ALICE',password=password)],mode='production')
    for stored in ['', 'plain', '$2b$04$......................invalid', '$2a$broken']:
        add([new(fields={'0':'alice','1':stored}),dict(op='store',id='a'),dict(op='login',id='login',username='alice',password='password'),
             dict(op='password',stored=stored,password='password')],mode='production')
    for password in ['', 'password','a\x00b','éΩ😀','a'*71,'a'*72,'a'*73,'a'*100]:
        for prefix in ['$2a$04$', '$2$04$', '$2a$31$']:
            cases.append(dict(kind='hash',password=password,salt=prefix+'......................'))
    for cost in [4,5,6,12,31]:
        for salt in [[0]*16,[255]*16,list(range(16))]:cases.append(dict(kind='salt',cost=cost,bytes=salt))
    for salt in ['$2a$12$......................','$2a$05$abcdefghijklmnopqrstuu','$2a$04!......................',
                 '', '$2b$04$......................', '$2a$03$......................', '$2a$32$......................', '$2a$04$short']:
        cases.append(dict(kind='hash',password='password',salt=salt))
    def configuration(use=False,persist=False,admin=True,**database):
        values=dict(address='127.0.0.1:3307',name='game',username='test',password='synthetic',port='9999')
        values.update(database)
        return dict(op='configure',use_auth=use,persist_accounts=persist,default_admin=admin,database=values)
    def factory(actions):cases.append(dict(kind='factory',now=1700000000123,actions=actions))
    factory([dict(op='inspect')])
    for use in [False,True]:
        for persist in [False,True]:
            for admin in [False,True]:
                actions=[configuration(use,persist,admin)]
                if not persist or not use:
                    actions += [dict(op='login',username='alice',password='password'),dict(op='create',username='bob',password='password')]
                factory(actions)
    fields=['address','name','username','password']
    for mask in range(16):
        missing={name:None for i,name in enumerate(fields) if mask&(1<<i)}
        factory([configuration(True,True,**missing)])
    for host,database in [('localhost','game'),('127.0.0.1:49152','data'),('[::1]:3307','game'),('',''),('bad address','game?extra=1')]:
        factory([configuration(True,True,address=host,name=database,username='user@:/Ω',password='pass@:/Ω')])
    for previous in [False,True]:
        factory([configuration(previous,False),dict(op='create',username='alice',password='password'),
                 configuration(True,True,False,password=None),dict(op='login',username='alice',password='password'),
                 dict(op='create',username='bob',password='password'),configuration(False,False),
                 dict(op='login',username='alice',password='password')])
    factory([configuration(True,True,password=None),configuration(True,True),configuration(False,True),
             dict(op='login',username='alice',password='password'),configuration(False,False,False),dict(op='create',username='alice',password='password')])
    def timed(**options):
        case=dict(kind='clock',mode='production',operation='login',now=1000,existing=True,username='alice',password='password',ban=0,online=False,hashDelay=0,verifyDelay=10,getDelay=7,failHash=False)
        case.update(options);cases.append(case)
    for ban in [999,1000,1007,1010,1017,1018]:
        for online in [False,True]:
            for password in ['password','wrong']:timed(ban=ban,online=online,password=password)
    for mode in ['production','development']:
        for fail in [False,True]:timed(mode=mode,operation='create',existing=False,hashDelay=9,failHash=fail)
        for existing in [False,True]:
            for username in ['alice','Alice']:timed(mode=mode,existing=existing,username=username)
        timed(mode=mode,operation='update',hashDelay=9,password='next')
    for now in [-1000,9223372036854775802]:timed(now=now,operation='create',existing=False,hashDelay=9)
    def admission(**options):
        timed(**options);cases[-1].update(kind='admission',parseResponse=2,info=True)
    for response in range(23):
        for info in [False,True]:
            admission();cases[-1].update(parseResponse=response,info=info)
    for mode in ['production','development']:
        for password in ['password','wrong']:
            for ban in [0,1017,1018]:
                for online in [False,True]:admission(mode=mode,password=password,ban=ban,online=online)
        for failure in ['taken','get','store']:
            for existing in [False,True]:admission(mode=mode,failStorage=failure,existing=existing)
    return cases

def generate(check,interop_file=None):
    lock=json.loads(read(ROOT/'upstream.lock.json'))
    for part in ['client','server']:
        folder=ROOT/'reference'/part
        if run(['git','-C',folder,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('reference pin differs')
        run(['git','-C',folder,'diff','--quiet','HEAD','--'])
    jars=compiler.artifacts();RUN.mkdir(parents=True,exist_ok=True)
    artifact=json.loads(read(HERE/'json.lock.json'));json_jar=RUN/'json-simple-1.1.1.jar'
    if not json_jar.is_file() or hashlib.sha256(json_jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60) as response:data=response.read()
        if len(data)!=artifact['size'] or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        json_jar.write_bytes(data)
    classes=RUN/'classes';classes.mkdir(exist_ok=True)
    java=[SERVER/'core/game/system/security/EncryptionManager.java',HERE/'SystemManager.java']
    for source in java:read(source)
    run(['javac','--release','11','-encoding','UTF-8','-d',classes,*java])
    sources=[]
    for name in ['core/auth/Auth.kt','core/auth/AuthResponse.kt','core/auth/AuthProvider.kt','core/auth/UserAccountInfo.kt','core/auth/DevelopmentAuthenticator.kt','core/auth/ProductionAuthenticator.kt','core/storage/AccountStorageProvider.kt','core/storage/InMemoryStorageProvider.kt']:
        source=SERVER/name;read(source);sources.append(source)
    for name in ['Clock.kt','Player.kt','Repository.kt','SqlStorage.kt','AuthOracle.kt','AdmissionSupport.kt']:
        source=HERE/name;read(source);sources.append(source)
    constants=read(SERVER/'core/ServerConstants.kt')
    if 'var USE_AUTH = false' not in constants or 'var PERSIST_ACCOUNTS = false' not in constants:raise RuntimeError('review changed default auth configuration')
    config=RUN/'ServerConstants.kt'
    config.write_text('package core\nobject ServerConstants {var USE_AUTH=false;var PERSIST_ACCOUNTS=false;var NOAUTH_DEFAULT_ADMIN=true;var DATABASE_ADDRESS:String?=null;var DATABASE_NAME:String?=null;var DATABASE_USER:String?=null;var DATABASE_PASS:String?=null;var DATABASE_PORT:String?=null}',encoding='utf-8')
    sources.append(config)
    event=read(SERVER/'core/net/event/LoginReadEvent.kt')
    prefix=event[event.index('val (response, info) = Login.decodeFromBuffer(buffer)'):event.index('val details = PlayerDetails(info.username)')]
    gate=RUN/'AdmissionGate.kt'
    gate.write_text('import core.auth.*\nimport java.nio.ByteBuffer\nimport AdmissionLogin as Login\nimport AdmissionWorld as GameWorld\nobject AdmissionGate {fun read(session:AdmissionSession,buffer:ByteBuffer){try{\n'+prefix+'\nsession.ready=accountInfo\n}catch(e:Exception){session.write(AuthResponse.UnexpectedError)}}}',encoding='utf-8')
    sources.append(gate)
    stdlib=next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    classpath=os.pathsep.join(map(str,[classes,stdlib,json_jar]))
    run(['java','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',classpath,'-d',classes,*sources])
    def evaluate(cases):
        command=['java','-Dfile.encoding=UTF-8','-cp',classpath,'AuthOracleKt']
        if compiler.JAVA_HOME:command[0]=str(compiler.JAVA_HOME/'bin'/('java.exe' if os.name=='nt' else 'java'))
        result=subprocess.run(command,cwd=ROOT,input=''.join(json.dumps(case,ensure_ascii=True)+'\n' for case in cases).encode(),capture_output=True)
        if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
        return [json.loads(line) for line in result.stdout.splitlines() if line]
    output=evaluate(requests());verification=[]
    for case in output:
        if case['kind']=='hash' and not case['error']:
            for password in [case['password'],case['password']+'wrong']:
                verification.append(dict(kind='verify',password=password,stored=case['expected']))
    for stored in ['', 'plain','$2b$04$......................invalid','$2a$broken']:verification.append(dict(kind='verify',password='password',stored=stored))
    output+=evaluate(verification)
    data=''.join(json.dumps(case,ensure_ascii=True,sort_keys=True,separators=(',',':'))+'\n' for case in output).encode()
    read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(data).hexdigest(),
        sourceHashAlgorithm='sha256, CR removed',kotlinCompiler='1.8.20',
        scope='Complete original Auth.configure, UserAccountInfo, in-memory storage, development/production auth rules and EncryptionManager/BCrypt, plus verbatim LoginReadEvent prefix through authentication. Explicit controlled clock including elapsed password/storage work, hash entropy/cost, constants, player-password, packet-decoder/world-boundary and failing-provider adapters. SQL configuration fields observed through adapter; SQL execution is unsupported and throws. No communication/world/playable login claim.')
    folder=ROOT/'fixtures/auth'
    if check:
        if (folder/'oracle.jsonl').read_bytes().replace(b'\r',b'')!=data or json.loads((folder/'provenance.json').read_bytes())!=provenance:raise RuntimeError('Account oracle/provenance differs; inspect before regeneration')
        print(f'{len(output)} original Java/Kotlin account cases reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2,sort_keys=True)+'\n',encoding='utf-8')
        print(f'Generated {len(output)} original Java/Kotlin account cases.')
    if interop_file:
        supplied=[json.loads(line) for line in interop_file.read_text(encoding='utf-8').splitlines() if line]
        forward=evaluate(supplied)
        if not all(case['expected'] is True and not case['error'] for case in forward):raise RuntimeError('JVM rejected native default-cost hash')
        reverse=evaluate([dict(kind='hash_random',password=case['password']) for case in supplied])
        version=run(['java','-version']).stderr.decode().strip()
        print(json.dumps(dict(nativeToJvm=forward,jvmToNative=reverse,javaVersion=version),ensure_ascii=True,sort_keys=True))

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true');parser.add_argument('--java-home',type=pathlib.Path)
    parser.add_argument('--interop-file',type=pathlib.Path)
    args=parser.parse_args();compiler.JAVA_HOME=args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check,args.interop_file)
