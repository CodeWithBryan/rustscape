"""Real original/native SQL comparisons using the pinned schema and owned MariaDB."""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import random
import subprocess
import urllib.request
from database import Database,TOOLS

ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/sql-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('login_compiler',ROOT/'oracle/login/generate.py');compiler=importlib.util.module_from_spec(spec);spec.loader.exec_module(compiler)
read=compiler.read;run=compiler.run

def requests():
    cases=[]
    def add(actions,now=1700000000123):cases.append(dict(kind='sequence',now=now,actions=actions))
    def new(id='a',fields=None):return dict(op='new',id=id,fields={'0':'alice','1':'plain','3':2,'5':'127.0.0.1',**(fields or {})})
    def set_(field,value,id='a'):return dict(op='set',id=id,field=field,value=value)
    add([dict(op='get',id='missing',key='missing'),dict(op='taken',key='missing'),new(fields={'6':'different'}),dict(op='store',id='a'),dict(op='taken',key='ALICE'),dict(op='get',id='read',key='ALICE'),dict(op='ip',key='127.0.0.1'),dict(op='remove',id='read'),dict(op='get',id='gone',key='alice')])
    for field,value in [(0,'renamed'),(1,'next'),(2,900),(3,1),(4,17),(5,'10.0.0.1'),(6,'10.0.0.2'),(7,-1),(8,2**63-1),(9,'{bob,1}'),(10,'eve'),(11,'clan'),(12,'current'),(13,'1,1,1,1'),(14,-17),(15,1700000000000),(16,True),(17,dict(millis=1700000000999,nanos=999999999))]:
        add([new(),dict(op='store',id='a'),set_(field,value),dict(op='update',id='a'),dict(op='get',id='read',key='renamed'if field==0 else'alice')])
    add([new(),dict(op='store',id='a'),dict(op='update',id='a'),dict(op='timestamp_time',id='a',value=1700000000999),dict(op='update',id='a'),dict(op='get',id='read',key='alice')])
    for fields in [{},{'0':'only_name'},{'0':'with_pass','1':'password'}]:
        add([dict(op='new',id='a',fields=fields),dict(op='store',id='a'),dict(op='update',id='a')])
    add([new(),dict(op='store',id='a'),new('dup'),dict(op='store',id='dup'),dict(op='get',id='dup_read',key='Alice')])
    add([new(),dict(op='uninitialize',id='a'),dict(op='update',id='a')])
    for nanos in [0,123456789,999499999,999999999]:add([new(fields={'17':dict(millis=1700000000123,nanos=nanos)}),dict(op='store',id='a'),dict(op='get',id='read',key='alice')])
    for online in [-2,-1,0,1,2]:
        add([dict(op='sql',value=f"INSERT INTO members(username,password,online,muteTime,banTime,timePlayed,lastLogin)VALUES('alice',NULL,{online},-1,-2,-3,-4)"),dict(op='get',id='read',key='Alice')])
    add([dict(op='sql',value="INSERT INTO members(username,lastGameIp)VALUES(NULL,'127.0.0.1'),('alice','127.0.0.1')"),dict(op='ip',key='127.0.0.1')])
    add([dict(op='sql',value='ALTER TABLE members AUTO_INCREMENT=2147483647'),new(),dict(op='store',id='a'),new('b',{'0':'bob'}),dict(op='store',id='b'),dict(op='get',id='read',key='bob')])
    add([dict(op='sql',value='DROP TABLE members'),new(),dict(op='store',id='a'),dict(op='get',id='read',key='alice'),dict(op='ip',key='127.0.0.1')])
    add([dict(op='friends',key='alice'),dict(op='sql',value="CREATE TABLE players(username VARCHAR(15),contacts LONGTEXT)"),dict(op='friends',key='alice'),dict(op='sql',value="INSERT INTO players VALUES('alice','{bob,1}~{eve,2}~{adam,3}')"),dict(op='online',value=['eve','adam']),dict(op='friends',key='alice')])
    add([dict(op='sql',value="CREATE TABLE players(username VARCHAR(15),contacts LONGTEXT)"),dict(op='sql',value="INSERT INTO players VALUES('alice',NULL)"),dict(op='friends',key='alice')])
    for text in ['', '~',',','{}','{alice,1}', '{alice,1}~{bob,2}~{alice,3}', '{alice,1}~','~{alice,1}','{alice,+1}','{alice,-0}','{alice,10}','{alice,-1}','{alice, 1}','{alice,١}','{alice,１}','{alice,1,ignored}','{alice,}',',,']:
        cases.append(dict(kind='contacts',text=text))
    for size in [0,1,7,8,9,10,11,12,13,24,25,48,49,64,128,200,256]:
        colliders=[''.join('Aa'if (i>>bit)&1 else'BB' for bit in range(8))for i in range(256)]
        keys=colliders[:size];cases.append(dict(kind='map_order',keys=keys));cases.append(dict(kind='map_order',keys=list(reversed(keys))))
        cases.append(dict(kind='contacts',text='~'.join('{'+key+',1}'for key in keys)))
    rng=random.Random(530)
    for size in [12,25,49,129,512]:
        keys=[f'key_{rng.randrange(10000)}_😀' for _ in range(size)];cases.append(dict(kind='map_order',keys=keys))
    def spread(text):
        value=0
        units=text.encode('utf-16-be')
        for i in range(0,len(units),2):value=(value*31+int.from_bytes(units[i:i+2],'big'))&0xffffffff
        return value^(value>>16)
    bucket=[f'bucket_{i}' for i in range(10000) if spread(f'bucket_{i}')&63==0][:64]
    for count in [8,16,32,64]:
        keys=bucket[:count]+[f'filler_{i}'for i in range(160)]+bucket[:count]
        cases.append(dict(kind='map_order',keys=keys))
    for indices in [[],list(range(18)),[2,5,17],[-1,999],*[ [i] for i in range(18)]]:cases.append(dict(kind='update_query',indices=indices))
    return cases

def generate(args):
    lock=json.loads(read(ROOT/'upstream.lock.json'))
    for component in ['server','client']:
        folder=ROOT/'reference'/component
        if run(['git','-C',folder,'rev-parse','HEAD']).stdout.decode().strip()!=lock[component]['commit']:raise RuntimeError('Reference differs from pin')
        run(['git','-C',folder,'diff','--quiet','HEAD','--'])
    schema=run(['git','-C',ROOT/'reference/server','show','HEAD:Server/db_exports/global.sql']).stdout.replace(b'\r',b'')
    if schema!=(ROOT/'fixtures/sql/schema.sql').read_bytes().replace(b'\r',b''):raise RuntimeError('Pinned SQL schema differs')
    read(ROOT/'fixtures/sql/schema.sql');schema_record=json.loads(read(ROOT/'fixtures/sql/schema.provenance.json'))
    blob=run(['git','-C',ROOT/'reference/server','rev-parse','HEAD:Server/db_exports/global.sql']).stdout.decode().strip()
    if schema_record['blob']!=blob or schema_record['sha256']!=hashlib.sha256(schema).hexdigest():raise RuntimeError('SQL schema provenance differs')
    jars=compiler.artifacts();RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    json_lock=json.loads(read(ROOT/'oracle/auth/json.lock.json'));json_jar=RUN/'json-simple-1.1.1.jar'
    if not json_jar.is_file() or hashlib.sha256(json_jar.read_bytes()).hexdigest()!=json_lock['sha256']:
        with urllib.request.urlopen(json_lock['url'],timeout=60)as response:data=response.read()
        if len(data)!=json_lock['size']or hashlib.sha256(data).hexdigest()!=json_lock['sha256']:raise RuntimeError('JSON artifact mismatch')
        json_jar.write_bytes(data)
    for filename in ['artifacts.lock.json','database.py','generate.py','SqlOracle.kt','Clock.kt','StorageClock.kt','Repository.kt']:read(HERE/filename)
    source=read(SERVER/'core/game/system/communication/CommunicationInfo.java')
    selected=compiler.method(source,'public static HashMap<String, Contact> parseContacts(')
    wrapper=RUN/'CommunicationInfo.java';wrapper.write_text('package core.game.system.communication;import java.util.HashMap;public class CommunicationInfo{'+selected+'}',encoding='utf-8')
    java=[wrapper,SERVER/'core/game/system/communication/Contact.java',SERVER/'core/game/system/communication/ClanRank.java']
    for source in java[1:]:read(source)
    run(['javac','--release','11','-encoding','UTF-8','-d',classes,*java])
    kotlin=[SERVER/name for name in ['core/auth/UserAccountInfo.kt','core/storage/AccountStorageProvider.kt','core/storage/SQLStorageProvider.kt']]+[HERE/name for name in ['SqlOracle.kt','Clock.kt','StorageClock.kt','Repository.kt']]
    for source in kotlin:read(source)
    stdlib=next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    classpath=os.pathsep.join(map(str,[classes,stdlib,json_jar,TOOLS/'mysql-connector-java-8.0.29.jar',TOOLS/'protobuf-java-3.19.4.jar']))
    run(['java','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',classpath,'-d',classes,*kotlin])
    inputs=requests();encoded=''.join(json.dumps(case,ensure_ascii=True)+'\n'for case in inputs).encode();(RUN/'input.jsonl').write_bytes(encoded)
    outputs=None;modes=[]
    for tls in [False,True]:
        with Database(tls=tls)as db:
            command=['java','-Dfile.encoding=UTF-8','-Duser.timezone=UTC','-cp',classpath,'SqlOracleKt','127.0.0.1',str(db.port),'rs09_jvm',str(ROOT/'fixtures/sql/schema.sql'),str(db.root/'data'),'tls'if tls else'plaintext']
            if compiler.JAVA_HOME:command[0]=str(compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java'))
            result=subprocess.run(command,cwd=ROOT,input=encoded,capture_output=True,timeout=120)
            if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
            current=[json.loads(line)for line in result.stdout.splitlines()if line]
            if outputs is not None and current!=outputs:raise RuntimeError('Original SQL data differs between plaintext and TLS')
            outputs=current
            if args.native:
                config=dict(port=db.port,database='rs09_native',datadir=str(db.root/'data'),input=str(RUN/'input.jsonl'),output=str(RUN/'native.jsonl'),tls=tls)
                config_path=RUN/'native-config.json';config_path.write_text(json.dumps(config),encoding='utf-8')
                native=subprocess.run([str(args.native),'sql-native','--config',str(config_path)],cwd=ROOT,capture_output=True,timeout=120)
                if native.returncode:raise RuntimeError(native.stdout.decode(errors='replace')+native.stderr.decode(errors='replace'))
                actual=[json.loads(line)for line in (RUN/'native.jsonl').read_bytes().splitlines()if line]
                if actual!=outputs:
                    for i,(a,b)in enumerate(zip(actual,outputs)):
                        if a!=b:
                            (RUN/'difference.json').write_text(json.dumps(dict(index=i,tls=tls,native=a,jvm=b),indent=2,ensure_ascii=True),encoding='utf-8');raise RuntimeError(f'Original/native SQL case {i} differs; inspect runs/sql-oracle/difference.json')
                    raise RuntimeError('SQL case counts differ')
            server_version=db.query('SELECT VERSION();').strip()
        modes.append(dict(tls=tls,cases=len(outputs),serverStopped=True))
    expected=''.join(json.dumps(case,ensure_ascii=True,sort_keys=True,separators=(',',':'))+'\n'for case in outputs).encode()
    if args.native:(RUN/'difference.json').unlink(missing_ok=True)
    read(ROOT/'oracle/login/generate.py')
    metadata=dict(format=1,upstream=lock,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(outputs),fixtureSha256=hashlib.sha256(expected).hexdigest(),sourceHashAlgorithm='sha256, CR removed',
        scope='Complete original SQLStorageProvider and UserAccountInfo on real MariaDB with pinned schema. Original Contact/ClanRank and verbatim parseContacts method; controlled wall clock and online-player lookup. No game-world or playable-login claim.')
    fixtures=ROOT/'fixtures/sql'
    if args.check:
        if (fixtures/'oracle.jsonl').read_bytes().replace(b'\r',b'')!=expected or json.loads((fixtures/'provenance.json').read_bytes())!=metadata:raise RuntimeError('SQL oracle/provenance differs; inspect before regeneration')
        print(f'{len(outputs)} original SQL/collection cases reproduced over plaintext and TLS against {server_version}.')
    else:
        (fixtures/'oracle.jsonl').write_bytes(expected);(fixtures/'provenance.json').write_text(json.dumps(metadata,indent=2,sort_keys=True)+'\n',encoding='utf-8');print(f'Generated {len(outputs)} original SQL/collection cases.')
    if args.native:
        java_version=run(['java','-version']).stderr.decode().strip()
        openssl_version=subprocess.check_output(['openssl','version'],creationflags=subprocess.CREATE_NO_WINDOW if os.name=='nt'else 0).decode().strip()
        print(json.dumps(dict(cases=len(outputs),nativeMatched=True,server=server_version,serverStopped=True,modes=modes,javaVersion=java_version,opensslVersion=openssl_version),sort_keys=True))

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true');parser.add_argument('--java-home',type=pathlib.Path);parser.add_argument('--native',type=pathlib.Path)
    args=parser.parse_args();compiler.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None)
    generate(args)
