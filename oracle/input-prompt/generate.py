"""Compile unchanged modern input/BankUtils sources and exact prompt/packet bodies."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess,random,urllib.request
R=pathlib.Path(__file__).resolve().parents[2];H=pathlib.Path(__file__).parent;O=R/'runs/input-prompt-oracle';S=R/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',R/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
def value(kind,v):return dict(kind=kind,value=v)
def action(op,**kw):return dict(op=op,**kw)
def requests():
 rows=[]
 def add(actions,**kw):rows.append(dict(actions=actions,**kw))
 def setv(k,v,p=0):return action('set',player=p,key=k,value=v)
 def initial(kind=1,parse=False,script='ok'):
  return [setv('input-type',value('type',kind)),setv('parseamount',value('bool',parse)),setv('runscript',value('script',script))]
 texts=['','0','-1','+1','001','1k','1K','1kk','1m','1mm','1km','1\n',' 1 ','2147483647','2147483648','-2147483648','-2147483649','١٢','１２','𝟙','1K','1😀',[49,0xd800],[0xdc00]]
 vals=[value('text',v)for v in texts]+[value('int',i)for i in [-2147483648,-1,0,1,2147483647]]+[value('long',i)for i in [-1,0,1,2147483648]]+[value('bool',False)]
 for kind in range(5):
  for parse in [False,True]:
   for v in vals:add(initial(kind,parse)+[action('process',value=v)])
 for script in ['ok','false','unit','number','type','state']:
  for mode in ['process','receive']:
   for v in [value('int',0),value('text','12'),value('text','1kk'),value('text','bad')]:add(initial(0,True,script)+[action(mode,script=script,value=v)])
 for raw in [None,value('int',7),value('text','NUMERIC'),value('script','ok'),value('bool',True)]:
  for key in ['input-type','parseamount','runscript']:
   add(initial()+[setv(key,raw),action('receive',value=value('text','7'))])
 add([action('receive',value=value('text','2'))])
 add(initial()+[setv('runscript',value('after','wrong')),action('receive',value=value('text','2'))])
 add(initial()+[action('field',field='locked',value=True),action('receive',value=value('text','2'))])
 for mode in ['process','receive']:
  for err in ['ok','number','type','state']:
   for parse in [False,True]:
    v=dict(kind='print',value='12',label='object',error=err)
    add(initial(0,parse)+[action(mode,value=v)])
 for kind in range(5):
  add([action('send',type=kind,prompt=[0xd800,0,65535],script='unit')])
  for phase in ['set','interpreter','input','long','message']:
   for at in [1,2,3]:add([action('send',type=kind)],failures={phase:at})
 for mode in ['process','receive']:
  for script in ['ok','number','type','state']:
   for phase in ['get','invoke','dialogue','remove']:
    for at in range(1,5):add(initial(0,True,script)+[action(mode,value=value('text','12'))],failures={phase:at})
 for withdraw in [False,True]:
  for after in [False,True]:
   transfer=action('transfer',slot=-7,withdraw=withdraw,**({'after':'done'}if after else{}))
   for v in [value('int',0),value('text','0'),value('text','1k'),value('text','1kk'),value('text','2147483648'),value('text','bad')]:
    add([transfer,action('capture',name='x'),action('receive',value=v)])
   for phase in ['get','bank','take','deposit','last','after','set','interpreter','input','remove']:
    for at in [1,2,3]:add([transfer,action('receive',value=value('text','12'))],failures={phase:at})
   for phase in ['bank','take','deposit']:
    add([transfer,action('receive',value=value('text','12'))],hooks=[dict(event=phase,actions=[action('field',field='bank',value=1)])])
 add([action('transfer',slot=8,withdraw=False,after='done'),action('capture',name='old'),action('restore',player=1,name='old'),action('receive',player=1,value=value('text','5')),action('field',field='bank',value=2),action('call',name='old',value=value('text','7'))])
 add(initial()+[action('receive',value=value('text','4'))],hooks=[dict(event='invoke',actions=[action('send',type=4,script='unit')])])
 add([action('send',type=0)],hooks=[dict(event='interpreter',actions=[action('field',field='interpreter',value=2)])])
 add(initial()+[action('receive',value=value('text','4'))],hooks=[dict(event='locks',actions=[action('field',field='locks',value=2),action('field',player=2,field='locked',value=True)])])
 rng=random.Random(53024465)
 for op in [244,65,23]:
  for size in range(20):
   for data in [[b]*size for b in [0,127,128,255]]+[[rng.randrange(256)for _ in range(size)]]:
    rows.append(dict(decode=dict(opcode=op,data=data,start=0)))
 for wire_value in [-2**63,-1,0,1,37,1047,37**12-1,37**12,2**63-1]:
  payload=list(wire_value.to_bytes(8,'big',signed=True))
  for prefix in [[],[99],[1,2,3]]:rows.append(dict(decode=dict(opcode=244,data=prefix+payload+[8,9],start=len(prefix))))
 for payload in [[],[0],list(b'1k\0'),list(b'banking test\0'),list(range(1,256))+[0],[255,128,127,0,1,2],[49,0,50,0]]:
  for prefix in [[],[78,21]]:rows.append(dict(decode=dict(opcode=65,data=prefix+payload,start=len(prefix))))
 add([action('transfer',slot=8),action('callbackText')])
 for next_type in range(1,5):add([action('transfer',slot=8),action('capture',name='old'),action('send',type=next_type,script='unit'),action('receive',value=value('text','1k')),action('call',name='old',value=value('text','12'))])
 add([action('transfer'),action('capture',name='old'),action('removeRaw',key='runscript'),action('receive',value=value('text','1k')),action('call',name='old',value=value('text','9'))])
 return rows
def generate(check):
 lock=json.loads(c.read(R/'upstream.lock.json'));assert c.run(['git','-C',R/'reference/server','rev-parse','HEAD']).stdout.decode().strip()==lock['server']['commit'];c.run(['git','-C',R/'reference/server','diff','--quiet','HEAD','--'])
 O.mkdir(exist_ok=True);classes=O/'classes';classes.mkdir(exist_ok=True);java=[];kt=[]
 def adapter(name,text):
  p=O/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(java if name.endswith('.java')else kt).append(p)
 for name in ['core/net/packet/in/RunScript.kt','core/api/InputType.kt','content/global/handlers/iface/bank/BankUtils.kt']:
  p=S/name;c.read(p);kt.append(p)
 api=c.read(S/'core/api/ContentAPI.kt');prompts=c.method(api,'fun sendInputDialogue(player: Player, type: InputType,')
 processor=c.read(S/'core/net/packet/PacketProcessor.kt');branch=c.method(processor,'is Packet.InputPromptResponse ->').split('{',1)[1][:-1]
 adapter('core/api/Helpers.kt',c.read(H/'Helpers.kt').replace('/* ORIGINAL_PROMPTS */',prompts))
 adapter('oracle/Bridge.kt',c.read(H/'Bridge.kt').replace('/* ORIGINAL_INPUT_BRANCH */',branch))
 decoder=c.read(S/'core/net/packet/in/Decoders530.kt');methods=[]
 for opcode,name in [(244,'INPUT_SHORT_STRING_RESPONSE'),(65,'INPUT_LONG_STRING_RESPONSE'),(23,'INPUT_INT_RESPONSE')]:
  source=decoder[decoder.index(name+'('):];method=c.method(source,'override fun decode(')
  methods.append('class Decoder'+str(opcode)+':Decoder {'+method+'}')
 adapter('oracle/Wire.kt',c.read(H/'Wire.kt').replace('/* ORIGINAL_DECODERS */','\n'.join(methods)))
 buffer=c.read(S/'core/net/packet/IoBuffer.java');methods='\n'.join(c.method(buffer,'public '+signature)for signature in ['long getLong()','int getInt()','String getString()'])
 adapter('core/net/packet/IoBuffer.java','package core.net.packet;import java.nio.ByteBuffer;import core.cache.misc.buffer.ByteBufferUtils;public class IoBuffer{private ByteBuffer buf;public IoBuffer(byte[]data){buf=ByteBuffer.wrap(data);}public int position(){return buf.position();}public void position(int n){buf.position(n);}'+methods+'}')
 strings=c.read(S/'core/tools/StringUtils.java');valid=next(line.strip()for line in strings.splitlines()if 'char[] VALID_CHARS ='in line)
 adapter('core/tools/StringUtils.java','package core.tools;public class StringUtils{'+valid+c.method(strings,'public static String longToString(long l)')+'}')
 strings=c.read(S/'core/cache/misc/buffer/ByteBufferUtils.kt');adapter('core/cache/misc/buffer/ByteBufferUtils.kt','package core.cache.misc.buffer\nimport java.nio.ByteBuffer\nobject ByteBufferUtils {\n@JvmStatic\n'+c.method(strings,'fun getString(buffer: ByteBuffer): String')+'\n}')
 for name in ['Player.java','Oracle.java']:p=H/name;c.read(p);java.append(p)
 jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jlock=json.loads(c.read(R/'oracle/auth/json.lock.json'));jsonjar=R/'runs/interfaces-oracle/json-simple-1.1.1.jar';assert hashlib.sha256(jsonjar.read_bytes()).hexdigest()==jlock['sha256']
 pom=c.read(R/'reference/server/Server/pom.xml');reflect=json.loads(c.read(H/'reflect.lock.json'));assert '<artifactId>kotlin-reflect</artifactId>\n      <version>'+reflect['version']+'</version>'in pom
 reflectjar=O/('kotlin-reflect-'+reflect['version']+'.jar')
 if not reflectjar.exists()or reflectjar.stat().st_size!=reflect['size']or hashlib.sha256(reflectjar.read_bytes()).hexdigest()!=reflect['sha256']:
  data=urllib.request.urlopen(reflect['url'],timeout=30).read();assert len(data)==reflect['size']and hashlib.sha256(data).hexdigest()==reflect['sha256'];reflectjar.write_bytes(data)
 cp=os.pathsep.join(map(str,[classes,std,jsonjar,reflectjar]))
 c.run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*kt,*java]);c.run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 rows=requests();data=''.join(json.dumps(row,ensure_ascii=True)+'\n'for row in rows).encode();exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 p=subprocess.run(list(map(str,[exe,'-Xmx768m','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Oracle'])),cwd=R,input=data,capture_output=True,timeout=60);assert p.returncode==0,p.stderr.decode(errors='replace');out=[json.loads(line)for line in p.stdout.splitlines()];assert len(out)==len(rows)
 fixture=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in out).encode();c.read(pathlib.Path(__file__))
 initial=json.loads(c.read(H/'initial.lock.json'));assert hashlib.sha256(b''.join(fixture.splitlines(keepends=True)[:initial['caseCount']])).hexdigest()==initial['sha256']
 wire=json.loads(c.read(H/'wire.lock.json'));assert hashlib.sha256(b''.join(fixture.splitlines(keepends=True)[:wire['caseCount']])).hexdigest()==wire['sha256']
 provenance=dict(format=1,initialBaseline=initial,wireBaseline=wire,declaredReflectionArtifact=reflect,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(fixture).hexdigest(),scope='Complete unchanged modern RunScript.kt, InputType and BankUtils; exact selected ContentAPI prompt/PacketProcessor input branch, three Decoders530 input methods and IoBuffer/StringUtils/ByteBufferUtils helper methods. Player/attributes/locks/interpreter/bank/callback APIs are mutable recording services; not full Entity/events, bank item mutation, dialogue rendering, canonical callbacks or live acceptance.')
 folder=R/'fixtures/input-prompt';folder.mkdir(exist_ok=True)
 if check:assert(folder/'oracle.jsonl').read_bytes()==fixture and json.loads((folder/'provenance.json').read_text(encoding='utf-8'))==provenance
 else:(folder/'oracle.jsonl').write_bytes(fixture);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
 print(len(rows),'original input/BankUtils scenarios reproduced'if check else'original input/BankUtils scenarios generated')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
