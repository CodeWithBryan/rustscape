"""Verbatim server item decoder/template methods, with isolated cache inputs."""
import argparse, hashlib, importlib.util, json, os, pathlib, random, re, subprocess, urllib.request
ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = pathlib.Path(os.environ.get('RS09_ITEM_ORACLE_RUN', ROOT / 'runs/item-oracle'))
SERVER = ROOT / 'reference/server/Server/src/main'
spec = importlib.util.spec_from_file_location('compiler', ROOT / 'oracle/login/generate.py')
compiler = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compiler)
read, run = compiler.read, compiler.run

METHODS = [
    'public static void parse()', 'public static ItemDefinition forId(',
    'public static ItemDefinition parseDefinition(', 'public static void defineTemplates()',
    'public void transferNoteDefinition(', 'public void transferLendDefinition(',
    'public void transferRecolourDefinition(', 'public void setStackable(',
    'public void setMaleWornModelId3(', 'public void setMaleWornModelId4(',
    'public void setFemaleWornModelId3(', 'public void setFemaleWornModelId4(',
    'public static Map<Integer, ItemDefinition> getDefinitions()',
    'public boolean isStackable()', 'public boolean isPlayerType()',
    'public boolean hasAction(', 'public boolean hasDestroyAction()',
    'public boolean hasWearAction()', 'public boolean hasSpecialBar()',
    'public int getQuestId()', 'public int getArchiveId()', 'public int getFileId()',
]

def requests():
    rng = random.Random(530)
    cases = []
    def add(data, item=530): cases.append(dict(kind='decode', id=item, data=bytes(data).hex()))
    add([0], -1)
    add([], -2**31)
    for opcode in range(256):
        # Unknown codes return at that byte; known codes may underflow.
        add([opcode, 255, 128, 1, 0, 0, 0, 0, 0])
    commands = [[0], [10], [11], [16], [65]]
    for opcode in [1,4,5,6,7,8,23,24,25,26,78,79,90,91,92,93,95,97,98,110,111,112,121,122]:
        for value in [0,1,32767,32768,65535]: commands.append([opcode, value >> 8, value & 255])
    for opcode in [96,113,114,115]:
        for value in [0,1,127,128,255]: commands.append([opcode,value])
    for opcode in [2,3,*range(30,40)]:
        for data in [b'',b'null',b'hidden',b'Examine',b'WEAR',bytes(range(1,256))]: commands.append([opcode,*data,0])
    for value in [-2**31,-1,0,1,2**31-1]: commands.append([12,*value.to_bytes(4,'big',signed=True)])
    for opcode in [40,41]:
        for length in [0,1,3,255]: commands.append([opcode,length,*[rng.randrange(256)for _ in range(length*4)]])
    for length in [0,1,3,255]: commands.append([42,length,*[rng.randrange(256)for _ in range(length)]])
    for opcode in range(100,110): commands.append([opcode,255,255,128,0])
    for opcode in [125,126,127,128,129,130]: commands.append([opcode,255,128,0])
    commands += [[249,0], [249,3,0,0,2,174,0,0,0,1,1,0,3,93,65,128,255,0,0,0,2,174,255,255,255,255]]
    for command in commands:
        add([*command,0])
        for stop in sorted(set([0,1,len(command)//2,len(command)-1])):
            add(command[:stop])
    for _ in range(180):
        data = sum(rng.choices(commands[1:],k=rng.randrange(1,15)),[]) + [0,255]
        add(data,rng.randrange(-2**31,2**31))

    fields = dict(id=10,name='base',interfaceModelId=123,modelZoom=456,modelRotationX=789,
        modelRotationY=1001,modelOffsetX=-101,modelOffsetY=102,stackable=True,value=100,
        membersOnly=True,maleWornModelId1=20,maleWornModelId2=21,femaleWornModelId1=30,
        femaleWornModelId2=31,maleWornModelId3=32,femaleWornModelId3=33,
        maleWornModelId4=34,femaleWornModelId4=35,groundActions=['ground',None,'take'],
        options=['first','wear','Destroy',None,'last'],originalModelColors=[1,-1],
        modifiedModelColors=[2,-2],textureColour1=[3,-3],textureColour2=[4,-4],
        unknownArray1=[-128,127],unknownArray2=[5,-5],unknownArray3=[[1],None,[]],
        unnoted=True,colourEquip1=103,colourEquip2=104,noteId=100,noteTemplateId=-1,
        stackIds=[1,2],stackAmounts=[3,4],teamId=-3,lendId=101,lendTemplateId=-1,
        recolourId=102,recolourTemplateId=-1,equipId=17,itemRequirements={'3':99},
        clientScriptData={'686':1,'861':123,'-1':'text'},itemType=-1,
        handlers={'examine':'Original examine','tradeable':False},examine='cached')
    other = dict(fields,id=11,name='template',value=200,options=['T'],modelZoom=111,
        modelOffsetX=-222,modelOffsetY=333,interfaceModelId=222,modelRotationX=333,
        modelRotationY=444,originalModelColors=[44],modifiedModelColors=[55],
        textureColour1=[66],textureColour2=[77],membersOnly=False,maleWornModelId1=-1)
    def sequence(initial,actions,**kwargs): cases.append(dict(kind='sequence',initial=initial,actions=actions,**kwargs))
    def op(name,**kwargs): return dict(op=name,**kwargs)
    for transfer in ['note','lend','recolour']:
        for target in range(3):
            for base in range(3):
                for template in range(3):
                    actions=[op(transfer,to=target,base=base,template=template),op('query',slot=target)]
                    for field,value in [('groundActions','mutated'),('options','changed'),('originalModelColors',-300),('modifiedModelColors',301),('textureColour1',-302),('textureColour2',303),('unknownArray1',-7)]:
                        actions.append(op('mutate',slot=base,field=field,index=0,value=value))
                    actions.append(op('mutate',slot=base,field='clientScriptData',key='686',value='1'))
                    sequence([dict(slot=0,fields={}),dict(slot=1,fields=fields),dict(slot=2,fields=other)],actions)
        for null in ['options','groundActions','originalModelColors','modifiedModelColors','textureColour1','textureColour2','unknownArray1','clientScriptData']:
            sequence([dict(slot=0,fields=fields),dict(slot=1,fields=dict(other,**{null:None}))],[op(transfer,to=0,base=1,template=1),op('query',slot=0)])
    for count in [0,1,2,3,4,12]:
        sequence([dict(slot=0,fields=dict(fields,noteId=2,noteTemplateId=1,lendId=1,lendTemplateId=2,recolourId=2,recolourTemplateId=1)),dict(slot=1,fields=other),dict(slot=2,fields=dict(fields,noteId=0,noteTemplateId=1))],[op('templates',count=count),op('missing',id=-1),op('missing',id=50)])
    sequence([dict(slot=2513,fields=dict(fields,equipId=10)),dict(slot=3140,fields=dict(other,equipId=30))],[op('templates',count=0),op('alias',slot=3140,source=2513),op('templates',count=3141)])
    for values in [{},{'0':'00','1':'02ff00','2':'0100'},{'0':'00','1':None,'2':'ff01'},{'0':'00','1':'00','2':'00'}]:
        sequence([dict(slot=1,fields=fields),dict(slot=9,fields=other)],[op('load',count=3,files=values),op('query',slot=1)])
    for data in [None,'00','ff','0b00']:
        sequence([],[op('load',count=14959,files={'14958':data})],select=[0,2513,3140,14957,14958])
    for actions in [None,[],[None],['null'],['Examine'],['WEAR','DiSsOlVe'],['wıeld','eQuıp'],['ſummon','attacK']]:
        sequence([dict(slot=0,fields=dict(fields,options=actions))],[op('query',slot=0)])
    for params in [None,{}, {'686':1,'861':-2**31},{'686':'1','861':'2'},{'686':True,'861':True},{'686':None,'861':None}]:
        sequence([dict(slot=0,fields=dict(fields,clientScriptData=params))],[op('query',slot=0)])
    # Retain compact real-cache input bytes under version control; full archive
    # audits use separate, hash-pinned asset verification in xtask.
    sample = HERE/'cache-samples.json'
    if sample.exists():
        for key,data in json.loads(read(sample)).items():add(bytes.fromhex(data),int(key))
    cases.append(dict(kind='casePairs'))
    for a,b in [('Σ','ς'),('σ','ς'),('İ','i'),('ı','I'),('ſ','s'),('K','k'),('ß','SS'),('ẞ','ß'),('ᾀ','ᾈ'),('𐐀','𐐨'),('😀','😀'),('',''),('wear','WEAR'),('wear','wıeld')]:
        cases.append(dict(kind='caseCompare',left=a,right=b))
    for _ in range(100):
        text=''.join(chr(rng.choice([65,90,97,122,0x130,0x131,0x17f,0x212a,0x3a3,0x3c2,0x1f80,0x1e9e,0x10400,0x10428,0x1f600]))for _ in range(rng.randrange(0,12)))
        cases.append(dict(kind='caseCompare',left=text,right=rng.choice([text.upper(),text.lower(),text.swapcase(),text+'a'])))
    return cases

def generate(check,audit_file=None):
    lock=json.loads(read(ROOT/'upstream.lock.json'))
    for part in ['server','client']:
        path=ROOT/'reference'/part
        if run(['git','-C',path,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
        run(['git','-C',path,'diff','--quiet','HEAD','--'])
    jars=compiler.artifacts();RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'));json_jar=RUN/'json-simple-1.1.1.jar'
    if not json_jar.exists()or hashlib.sha256(json_jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as response:data=response.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        json_jar.write_bytes(data)
    stdlib=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));classpath=os.pathsep.join(map(str,[classes,stdlib,json_jar]))
    run(['java','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',classpath,'-d',classes,SERVER/'core/cache/misc/buffer/ByteBufferUtils.kt'])
    read(SERVER/'core/cache/misc/buffer/ByteBufferUtils.kt')
    item=read(SERVER/'core/cache/def/impl/ItemDefinition.java')
    body=item[item.index('private static final Map<Integer, ItemDefinition> DEFINITIONS'):item.index('public ItemDefinition()')]
    body+='\n'+compiler.method(item,'public ItemDefinition()')
    body+='\n'+'\n'.join(compiler.method(item,m)for m in METHODS)
    imports='import java.util.*;import java.nio.ByteBuffer;import core.cache.Cache;import core.cache.def.Definition;import core.cache.misc.buffer.ByteBufferUtils;import core.game.node.item.Item;import core.game.interaction.OptionHandler;import core.game.system.config.ItemConfigParser;import core.tools.Log;import static core.api.ContentAPIKt.log;'
    config=read(SERVER/'core/game/system/config/ItemConfigParser.kt')
    trade=re.search(r'const val TRADEABLE = "[^"]+"',config).group().split('=',1)[1].strip()
    cache=read(SERVER/'core/cache/Cache.java');strings=read(SERVER/'core/tools/StringUtils.java')
    adapters={
        'core/cache/def/impl/ItemDefinition.java':'package core.cache.def.impl;'+imports+'public class ItemDefinition extends Definition<Item> {'+body+'}',
        'core/game/node/Node.java':'package core.game.node;public class Node {}',
        'core/game/node/item/Item.java':'package core.game.node.item;public class Item extends core.game.node.Node {}',
        'core/game/interaction/OptionHandler.java':'package core.game.interaction;public class OptionHandler {}',
        'core/tools/Log.java':'package core.tools;public class Log {public static int ERR=1;}',
        'core/api/ContentAPIKt.java':'package core.api;public class ContentAPIKt {public static void log(Object source,int level,String text){throw new AssertionError(text);}}',
        'core/game/system/config/ItemConfigParser.java':'package core.game.system.config;public class ItemConfigParser {public static String TRADEABLE='+trade+';}',
        'core/tools/StringUtils.java':'package core.tools;public class StringUtils {'+compiler.method(strings,'public static boolean isPlusN(')+'}',
        'core/cache/Cache.java':'package core.cache;public class Cache {public static int count;public static java.util.Map<Integer,byte[]> files=new java.util.HashMap<>();public static Index[] getIndexes(){Index[] indexes=new Index[20];indexes[19]=new Index();return indexes;}'+compiler.method(cache,'public static final int getItemDefinitionsSize()')+'public static class Index {public int getContainersSize(){return (count+255)/256;}public int getFilesSize(int last){return count-last*256;}public byte[] getFileData(int group,int file){return files.get(group*256+file);}}}',
    }
    java=[]
    for name,text in adapters.items():
        path=RUN/'adapters'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf-8');java.append(path)
    path=SERVER/'core/cache/def/Definition.java';read(path);java.append(path)
    read(HERE/'ItemOracle.java');java.append(HERE/'ItemOracle.java')
    run(['javac','--release','11','-encoding','UTF-8','-cp',classpath,'-d',classes,*java])
    command=['java','-Xmx2g','-Dfile.encoding=UTF-8','-cp',classpath,'ItemOracle']
    if compiler.JAVA_HOME:command[0]=str(compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java'))
    def evaluate(inputs):
        input_file=RUN/'input.jsonl';input_file.write_bytes(''.join(json.dumps(case,sort_keys=True,ensure_ascii=True)+'\n'for case in inputs).encode())
        with input_file.open('rb')as stdin:result=subprocess.run(command,cwd=ROOT,stdin=stdin,capture_output=True,timeout=120)
        if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
        output=[json.loads(line)for line in result.stdout.splitlines()if line]
        if len(output)!=len(inputs):raise RuntimeError('Missing item cases')
        return output
    output=evaluate(requests())
    data=''.join(json.dumps(case,sort_keys=True,ensure_ascii=True,separators=(',',':'))+'\n'for case in output).encode()
    read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py');read(ROOT/'tools/port_jdk_case.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Verbatim ItemDefinition stored fields, constructor, parseDefinition/parse/forId/defineTemplates, all note/lend/recolour transfers, stackability/player-type/actions/special-bar/quest/archive/file helpers. Unchanged complete Definition.java and ByteBufferUtils.kt. JDK 11 char upper/lower pairs for every UTF-16 code unit and original String.equalsIgnoreCase observations. Explicit isolated cache index/file adapter and numeric TRADEABLE constant; no content configurations, plugins, inventory/equipment/world or complete ItemDefinition class claim. Snapshots include mutable-array/map and definition identity, relative read positions, partial registry loads and failures.')
    folder=ROOT/'fixtures/items'
    if check:
        if (folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text())!=provenance:raise RuntimeError('Item oracle/provenance differs; review before regeneration')
        print(f'{len(output)} original item-definition cases reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(output)} original item-definition cases.')
    if audit_file:
        result=evaluate([json.loads(audit_file.read_text())])[0]
        destination=RUN/'audit-output.json';destination.write_text(json.dumps(result,sort_keys=True,separators=(',',':'))+'\n',encoding='utf-8')
        print(f'Full item archive observations written to {destination}')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);p.add_argument('--audit-file',type=pathlib.Path);args=p.parse_args()
    compiler.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None)
    generate(args.check,args.audit_file)
