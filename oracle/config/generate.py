"""Execute the complete pinned ServerConfigParser with owned files and explicit adapters."""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import re
import subprocess
import urllib.request

ROOT=pathlib.Path(__file__).resolve().parents[2]
HERE=pathlib.Path(__file__).resolve().parent
RUN=ROOT/'runs/config-oracle'
SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py')
compiler=importlib.util.module_from_spec(spec);spec.loader.exec_module(compiler)
read=compiler.read;run=compiler.run

def changed(text,key,value=None,remove=False):
    section,leaf=key.split('.',1);lines=text.splitlines();current='';out=[];found=False
    for line in lines:
        if re.match(r'\[.*\]',line):current=line.strip()[1:-1]
        if current==section and re.match(re.escape(leaf)+r'\s*=',line):
            found=True
            if not remove:out.append(leaf+' = '+value)
        else:out.append(line)
    if not found and not remove:out+=['['+section+']',leaf+' = '+value] if not any(line=='['+section+']' for line in lines)else []
    if not found and not remove and any(line=='['+section+']' for line in lines):
        at=out.index('['+section+']')+1;out.insert(at,leaf+' = '+value)
    return '\n'.join(out)+'\n'

def requests(source,base):
    cases=[]
    def add(text,**options):cases.append(dict(kind='config',actions=[dict(text=text,**options)]))
    add(base);add(base,source='path');add('');add('broken TOML=')
    types={}
    for getter,key in re.findall(r'data\.get(String|Boolean|Long|Path)\("([^"]+)"',source):types[key]=getter
    for key,kind in sorted(types.items()):
        add(changed(base,key,remove=True))
        candidates={'String':['""','"A@name/Ω"','true','1'], 'Boolean':['true','false','"false"','1'],
            'Long':['-9223372036854775808','9223372036854775807','2147483648','-2147483649','1.25','"3"'],
            'Path':['""','"@data/a//b"','"/root/a"',json.dumps(r'C:\game\file.xml'),'3']}[kind]
        for value in candidates:add(changed(base,key,value))
    optional=[]
    for key in re.findall(r'data\.get(?:Boolean|Long|String)\("([^"]+)",\s*(?:true|false|-?\d+L?|"[^"]*")\)',source):optional.append(key)
    minimal=base
    for key in optional:minimal=changed(minimal,key,remove=True)
    add(minimal)
    for value in ['true','false','"true"','1','[true]']:
        add(changed(base,'world.personalized_shops',value))
    cases.append(dict(kind='config',actions=[dict(text=base),dict(text=changed(changed(base,'world.personalized_shops','true'),'world.world_id','"bad"')),dict(text='broken TOML='),dict(text=base)]))
    escapes=[r'"\n"',r'"\\n"',r'"\\b"',r'"\\f"',r'"\\t"',r'"\\r"',r'"\\/"',r'"\u0041"',r'"\\u0041"',r'"\U00000041"',r'"\U0001F600"',r'"\u005cn"',r'"\u0000"',r'"C:\\cache\\"',r"'C:\raw\file.xml'",'"""\nfirst\\\n  next"""',r'"""first\\b next"""',r'"""\U00000041"""']
    for token in escapes:
        for key in ['server.secret_key','world.motw_text','paths.cache_path']:
            add(changed(base,key,token))
    for version in ['1.8.0_401','1.9','11.0.32','25.0.3','100.0','8','9.0','1.','', '١١.0']:
        add(base,javaVersion=version)
    cases.append(dict(kind='config',actions=[dict(text=base),dict(text=changed(changed(base,'world.name','"new world"'),'world.world_id','"bad"')),dict(text=minimal)]))
    paths=['','abc','/','//','a/b','a//b/','/absolute/file.xml',r'C:\game\file.xml',r'\\server\share\file','a/b\\c','a\\b/c','../data','./a','Ω/😀', 'a\x00/b']
    for text in paths:cases.append(dict(kind='path',text=text,separator=os.sep))
    for x in [-2147483648,-32769,-16385,-16384,-1,0,16383,16384,32768,2147483647]:
        for z in [-2147483648,-5,-1,0,3,4,2147483647]:cases.append(dict(kind='location',text=f'{x},{-x if x!=-2147483648 else 2147483647},{z}'))
    for text in ['1,2','1,2,3,4','1,2,3,x','1,2,3,',' 1,2,3','1, 2,3','+1,-2,+3','١,２,۳','𝟙,2,3','2147483648,2,3','1,,3','']:
        cases.append(dict(kind='location',text=text))
    return cases

def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json'))
    for part in ['client','server']:
        folder=ROOT/'reference'/part
        if run(['git','-C',folder,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin changed')
        run(['git','-C',folder,'diff','--quiet','HEAD','--'])
    RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    jars=compiler.artifacts();artifacts=json.loads(read(HERE/'artifacts.lock.json'))
    libraries=[]
    for name,artifact in artifacts.items():
        path=RUN/name
        if not path.is_file()or hashlib.sha256(path.read_bytes()).hexdigest()!=artifact['sha256']:
            with urllib.request.urlopen(artifact['url'],timeout=60)as response:data=response.read()
            if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('Configuration test artifact differs')
            path.write_bytes(data)
        if not name.endswith('-sources.jar'):libraries.append(path)
    pom=read(ROOT/'reference/server/Server/pom.xml')
    if '<artifactId>toml4j</artifactId>\n      <version>0.7.2</version>' not in pom:raise RuntimeError('Review changed TOML library version')
    if '<kotlin.compiler.jvmTarget>11</kotlin.compiler.jvmTarget>' not in pom:raise RuntimeError('Review changed reference runtime target')
    parser=SERVER/'core/game/system/config/ServerConfigParser.kt';source=read(parser)
    base=read(ROOT/'fixtures/config/default.conf')
    if base!=read(ROOT/'reference/server/Server/worldprops/default.conf'):raise RuntimeError('Pinned default configuration differs')
    selector=sorted(set(re.findall(r'ServerConstants\.(\w+) =',source)))
    (RUN/'fields.txt').write_text('\n'.join(selector),encoding='utf-8')
    location_source=read(SERVER/'core/game/world/map/Location.kt')
    nested=compiler.method(location_source,'value class LocationHash(')
    location=RUN/'Location.kt';location.write_text('package core.game.world.map\nclass Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,(z%4+4)%4);val x get()=hash.x;val y get()=hash.y;val z get()=hash.z;companion object{fun create(x:Int,y:Int,z:Int)=Location(x,y,z)}\n@JvmInline '+nested+'}',encoding='utf-8')
    levels=read(SERVER/'core/tools/SystemLogger.kt')
    enums=RUN/'Levels.kt';enums.write_text('package core.tools\n'+compiler.method(levels,'enum class LogLevel {')+'\n'+compiler.method(levels,'enum class Log {'),encoding='utf-8')
    api_source=read(SERVER/'core/api/ContentAPI.kt')
    api_config=RUN/'ContentConfig.kt';api_config.write_text('package core.api\nimport com.moandjiezana.toml.Toml\nimport core.game.system.config.ServerConfigParser\n'+compiler.method(api_source,'fun getServerConfig(): Toml'),encoding='utf-8')
    startup=read(SERVER/'core/Server.kt')
    tcp=re.search(r'NioReactor.configure\(([^\n]+)\)',startup).group(1)
    ws=startup[startup.index('val websocketPort = if'):startup.index('webSocketServer = GameWebSocketServer')]
    ports=RUN/'ListenerPorts.kt';ports.write_text('import core.ServerConstants\nimport core.game.world.GameWorld\nobject ListenerPorts{fun value():Any?{if(GameWorld.settings==null)return null;val tcp='+tcp+'\nval websocket=if(ServerConstants.WEBSOCKET_ENABLED){\n'+ws+'\nwebsocketPort\n}else null\nreturn listOf(tcp,websocket)}}',encoding='utf-8')
    java=SERVER/'core/tools/mysql/Database.java';read(java)
    run(['javac','--release','11','-encoding','UTF-8','-d',classes,java])
    sources=[parser,SERVER/'core/ServerConstants.kt',SERVER/'core/game/world/GameSettings.kt',SERVER/'core/tools/TickUtils.kt',location,enums,api_config,ports]
    sources+=[HERE/name for name in ['Api.kt','GameWorld.kt','Shutdown.kt','CastleWars.kt','ConfigOracle.kt']]
    for path in sources:
        if path.is_relative_to(SERVER)or path.is_relative_to(HERE):read(path)
    stdlib=next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    classpath=os.pathsep.join(map(str,[classes,stdlib,*libraries]))
    run(['java','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',classpath,'-d',classes,*sources])
    command=['java','-Dfile.encoding=UTF-8','-cp',classpath,'ConfigOracleKt',str(RUN/'fields.txt'),str(RUN/'input.conf')]
    if compiler.JAVA_HOME:command[0]=str(compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java'))
    result=subprocess.run(command,cwd=ROOT,input=''.join(json.dumps(case,ensure_ascii=True)+'\n'for case in requests(source,base)).encode(),capture_output=True,timeout=120)
    if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
    output=[json.loads(line)for line in result.stdout.splitlines()if line]
    encoded=''.join(json.dumps(case,ensure_ascii=True,sort_keys=True,separators=(',',':'))+'\n'for case in output).encode()
    read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py');read(ROOT/'oracle/auth/json.lock.json')
    metadata=dict(format=1,upstream=lock,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(encoded).hexdigest(),sourceHashAlgorithm='sha256, CR removed',
        scope='Complete original ServerConfigParser and ServerConstants classes compiled unchanged; original GameSettings constructor/getters and Database constructor exercised. Explicit log, world holder, shutdown, unrelated teleport initialization and Location-construction adapters; original nested LocationHash. JVM exit/version controlled by harness. No native startup/world gameplay or complete legacy TOML grammar claim.')
    folder=ROOT/'fixtures/config'
    if check:
        if(folder/'oracle.jsonl').read_bytes().replace(b'\r',b'')!=encoded or json.loads((folder/'provenance.json').read_bytes())!=metadata:raise RuntimeError('Configuration oracle/provenance differs; inspect before regeneration')
        print(f'{len(output)} original configuration cases reproduced.')
    else:
        (folder/'oracle.jsonl').write_bytes(encoded);(folder/'provenance.json').write_text(json.dumps(metadata,indent=2,sort_keys=True)+'\n',encoding='utf-8');print(f'Generated {len(output)} original configuration cases.')

if __name__=='__main__':
    args=argparse.ArgumentParser(description=__doc__);args.add_argument('--check',action='store_true');args.add_argument('--java-home',type=pathlib.Path);options=args.parse_args()
    compiler.JAVA_HOME=options.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None)
    generate(options.check)
