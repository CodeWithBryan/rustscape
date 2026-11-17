"""Execute original GameAttributes and verbatim profile attribute parse/save methods."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess,tempfile,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/attributes-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');compiler=importlib.util.module_from_spec(spec);spec.loader.exec_module(compiler)
read=compiler.read;run=compiler.run
def requests():
    cases=[]
    def add(actions,now=1000,step=0):cases.append(dict(actions=actions,now=now,step=step))
    def value(kind,data):return dict(kind=kind,value=data)
    def set_(key,kind,data):return dict(op='set',key=key,value=value(kind,data))
    def load(attrs):return dict(op='json',root=dict(attributes=attrs))
    def attr(kind,data,key='key',**extra):return dict(key=key,type=kind,value=data,**extra)
    def save():return dict(op='save')
    for key in ['key','/save:key','prefix/save:key','/save:/save:key','/save:','x/save:/save:y','😀','null','']:
        add([set_(key,'int',7),dict(op='get',key=key),dict(op='get',key=key.replace('/save:','')),save(),dict(op='remove',key=key),save()])
    for kind,values in [('int',[-2**31,-1,0,2**31-1]),('long',[-2**63,0,2**63-1]),('short',[-32768,32767]),('byte',[-128,-1,0,127]),('bool',[False,True]),('str',['','value','éΩ😀','/\n\x00\x7f\u2000']),('object',['opaque'])]:
        for v in values:add([set_('/save:key',kind,v),dict(op='get',key='key'),dict(op='get_or',key='absent',value=value('int',99)),save(),dict(op='json_roundtrip'),save()])
    for number in range(-128,128):add([set_('/save:key','byte',number),save(),dict(op='json_roundtrip'),save()])
    for kind,values in [('int',['0','+1','-2147483648','2147483648','１',' 1',None,True]),('long',['9223372036854775807','-9223372036854775808','9223372036854775808']),('short',['-32768','32767','32768','-32769']),('bool',[True,False,None,'true',0]),('str',[None,True,1,1.25,['/','\x80','\u2001',None],{'BB':'/','Aa':'\u2000','é':'😀'}]),('location',['[3200, 3200, 0]','32768,-32769,-1','[1, 2, 3, 4]','[1,2,3,bad]','1,2','[1,2,3,]',' [1,2,3] ','[١,２,-1]','[\u0085 1,2,3]','[\u00a0 1,2,3]']),('byte',['AA==','AB==','AA','AA=','AA===','AAA','AAA=','AAAA','AAAAAA==','AAAA?','',None,'AA==\n','_A==','A','A===']),('invalid',['anything'])]:
        for v in values:add([set_('/save:key','int',9),load([attr(kind,v,expirable=True,**{'expiration-time':'1001'})]),save()])
    for expiry in [-2**63,999,1000,1001,2**63-1,'bad',None]:
        for existing in [False,True]:
            actions=[set_('/save:key','int',3)]if existing else[]
            add(actions+[load([attr('int','7',expirable=True,**{'expiration-time':expiry})]),save(),dict(op='get',key='key')])
    add([load([attr('int','1',expirable=True,**{'expiration-time':1000}),attr('int','2',expirable=True,**{'expiration-time':1000})]),save()],step=1)
    for expirable in [None,'true',0,[],{}]:add([load([attr('int','1',expirable=expirable)]),save()])
    for bad in [None,True,{},'string',1]:add([load([attr('int','1','before'),bad,attr('int','3','after')]),save()])
    for root in [None,[],{},dict(attributes=None),dict(attributes=False),dict(attributes={}),dict(attributes=[])]:add([dict(op='json',root=root),save()])
    add([load([{},dict(value=None),dict(type='str'),dict(key=None,type=None)]),save()])
    add([load([attr('int','1'),attr('int','2'),attr('int','3')]),dict(op='remove',key='key'),save(),set_('key','str','new'),save(),dict(op='remove',key='/save:key'),save()])
    add([dict(op='raw',key=None,value=value('str','null key')),dict(op='saved',key=None),dict(op='expiry',key=None,value=None),save(),dict(op='json_roundtrip'),save()])
    add([set_('/save:key','str','live'),dict(op='expiry',key='key',value=999),dict(op='get',key='key'),save(),dict(op='remove',key='key'),set_('/save:key','str','new'),save()])
    add([set_('/save:key','location',[3200,3201,0]),dict(op='alias_location',key='key',value=[32768,-32769,-1]),save(),dict(op='get',key='key')])
    xmls=['<root/>','<root><GameAttribute key="a" type="int" value="1"/><GameAttribute key="b" type="string" value="hello"/></root>',
        '<GameAttribute key="a" type="long" value="-9223372036854775808"/>','<root><GameAttribute key="a" type="int" value="1"/><GameAttribute key="a" type="int" value="2"/></root>',
        '<root><GameAttribute key="first" type="int" value="1"/><GameAttribute key="bad" type="int" value="bad"/><GameAttribute key="after" type="int" value="2"/></root>',
        '<root><GameAttribute key="a" type="int" value="1"/></broken>','<root><GameAttribute key="a" type="int" value="1"/>',
        '<root><Group><GameAttribute key="nested" type="string" value="&lt;&amp;&quot;&#10;"/></Group></root>',
        '<root xmlns:p="urn:test"><p:GameAttribute key="ignored" type="int" value="7"/><GameAttribute xmlns="urn:x" key="seen" type="int" value="8"/></root>',
        '<root><!--<GameAttribute key="ignored"/>--><![CDATA[<GameAttribute/>]]><GameAttribute key="chars" type="string" value="a\nb\tc\r\nd"/></root>',
        '<!DOCTYPE root [<!ENTITY val "hello">]><root><GameAttribute key="entity" type="string" value="&val;"/></root>',
        '<root><GameAttribute type="string"/><GameAttribute key="unknown" type="wat" value="1"/></root>']
    for kind in ['bool','int','long','byte','short','string','str','unknown']:
        for v in ['','true','TRUE',' true ','false','-128','127','128','-32769','32768','2147483648','１']:
            xmls.append('<root><GameAttribute key="key" type="'+kind+'" value="'+v+'"/></root>')
    for xml in xmls:
        add([dict(op='xml',xml=xml,encoding='UTF-8'),save(),dict(op='json',root={}),save()])
    for encoding in ['UTF-8','UTF-16','UTF-16LE','UTF-16BE']:
        add([dict(op='xml',xml='<?xml version="1.0" encoding="'+encoding+'"?><root><GameAttribute key="é😀" type="string" value="Ωé😀"/></root>',encoding=encoding),save()])
    for encoding,data in [('ISO-8859-1','é\x80\x85'),('windows-1252','é€‚'),('Shift_JIS','日本語')]:
        add([dict(op='xml',xml='<?xml version="1.0" encoding="'+encoding+'"?><root><GameAttribute key="encoded" type="string" value="'+data+'"/></root>',encoding=encoding),save()])
    for ch in ['\t','\n','\u001c','\u0085','\u00a0','\u1680','\u2007','\u202f','\u3000']:
        add([load([attr('location','['+ch+'1'+ch+',2,3]')]),save()])
    for xml in ['<root><GameAttribute key="a" key="b" type="int" value="1"/></root>','<root><GameAttribute key="a" type="string" value="é😀"/></root><another/>','<root><gameattribute key="ignored"/><GameAttributeExtra key="ignored"/></root>']:
        add([dict(op='xml',xml=xml,encoding='UTF-8'),save()])
    add([set_('/save:key','int',3),dict(op='dump'),save()])
    return cases
def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json'))
    for part in ['server','client']:
        folder=ROOT/'reference'/part
        if run(['git','-C',folder,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
        run(['git','-C',folder,'diff','--quiet','HEAD','--'])
    jars=compiler.artifacts();RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'));json_jar=RUN/'json-simple-1.1.1.jar'
    if not json_jar.exists()or hashlib.sha256(json_jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as response:data=response.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        json_jar.write_bytes(data)
    stdlib=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));classpath=os.pathsep.join(map(str,[classes,stdlib,json_jar]))
    def write(name,text):path=RUN/name;path.write_text(text,encoding='utf-8');return path
    original=SERVER/'core/game/node/entity/impl/GameAttributes.java';read(original)
    constants=write('ServerConstants.java','package core;public class ServerConstants {public static String PLAYER_ATTRIBUTE_PATH="";}')
    clock=write('Clock.java','package oracle;public class Clock {public static long value,step;public static int reads;public static long now(){reads++;long result=value;value+=step;return result;}}')
    run(['javac','--release','11','-d',classes,original,constants,clock])
    location_source=read(SERVER/'core/game/world/map/Location.kt');nested=compiler.method(location_source,'value class LocationHash(');from_string=compiler.method(location_source,'fun fromString(')
    location=write('Location.kt','package core.game.world.map\nclass Location(x:Int,y:Int,z:Int=0){var hash=LocationHash(x,y,(z%4+4)%4);var x:Int get()=hash.x;set(v){hash=LocationHash(v,y,z)};var y:Int get()=hash.y;set(v){hash=LocationHash(x,v,z)};var z:Int get()=hash.z;set(v){hash=LocationHash(x,y,(v%4+4)%4)};override fun toString()="[$x, $y, $z]";companion object {fun create(x:Int,y:Int,z:Int)=Location(x,y,z);'+from_string+'}\n@JvmInline '+nested+'}')
    player=write('Player.java','package core.game.node.entity.player;public class Player {private final core.game.node.entity.impl.GameAttributes attrs=new core.game.node.entity.impl.GameAttributes();public core.game.node.entity.impl.GameAttributes getGameAttributes(){return attrs;}public String getName(){return "player";}}')
    log=write('Log.java','package core.tools;public enum Log {WARN}')
    api=write('ContentAPIKt.java','package core.api;public class ContentAPIKt {public static java.util.List<String> logs=new java.util.ArrayList<>();public static void log(Class<?> c,core.tools.Log l,String message){logs.add(message);}}')
    system=write('System.java','package core.game.node.entity.player.info.login;public class System {public static long currentTimeMillis(){return oracle.Clock.now();}}')
    run(['javac','--release','11','-cp',classpath,'-d',classes,player,log,api,system])
    parser=read(SERVER/'core/game/node/entity/player/info/login/PlayerSaveParser.kt');saver=read(SERVER/'core/game/node/entity/player/info/login/PlayerSaver.kt')
    methods=write('AttributeMethods.kt','package core.game.node.entity.player.info.login\nimport core.game.node.entity.player.Player\nimport core.game.world.map.Location\nimport core.tools.Log\nimport core.api.ContentAPIKt.log\nimport org.json.simple.*\nimport java.util.Base64\nclass AttributeParser(val player:Player){var saveFile:JSONObject?=null;'+compiler.method(parser,'fun parseAttributes(')+'}\nclass AttributeSaver(val player:Player){'+compiler.method(saver,'fun saveAttributes(')+'}')
    run(['java','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',classpath,'-d',classes,location,methods])
    read(HERE/'AttributeOracle.java');run(['javac','--release','11','-encoding','UTF-8','-cp',classpath,'-d',classes,HERE/'AttributeOracle.java'])
    inputs=requests();command=['java','-Dfile.encoding=UTF-8','-cp',classpath,'AttributeOracle']
    if compiler.JAVA_HOME:command[0]=str(compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java'))
    with tempfile.TemporaryDirectory(prefix='xml-',dir=RUN)as temp:
        assert pathlib.Path(temp).resolve().parent==RUN.resolve()
        result=subprocess.run(command+[temp],cwd=ROOT,input=''.join(json.dumps(case,sort_keys=True,ensure_ascii=True)+'\n'for case in inputs).encode(),capture_output=True)
    if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
    output=[json.loads(line)for line in result.stdout.splitlines()if line]
    if len(output)!=len(inputs):raise RuntimeError('Missing attribute cases')
    data=''.join(json.dumps(case,sort_keys=True,ensure_ascii=True,separators=(',',':'))+'\n'for case in output).encode()
    read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unmodified GameAttributes, including its DOM XML file loader, plus verbatim PlayerSaveParser.parseAttributes and PlayerSaver.saveAttributes. Explicit player, controlled clock and recording logger adapters; selected LocationHash/fromString with coordinate/identity holder, not world geometry. File paths isolated in owned temporary directories. No Entity event dispatch, complete profile, general XML/JDK object model or playable world claim.')
    folder=ROOT/'fixtures/attributes'
    if check:
        if (folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text())!=provenance:raise RuntimeError('Attribute oracle/provenance differs; review before regeneration')
        print(f'{len(output)} original attribute cases reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(output)} original attribute cases.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);args=p.parse_args();compiler.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(args.check)
