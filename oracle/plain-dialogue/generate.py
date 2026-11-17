"""Original DialUtils/plain dialogue/Continue differential corpus."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,re
H=pathlib.Path(__file__).resolve().parent;R=next(p for p in H.parents if(p/'upstream.lock.json').is_file());O=R/'runs/plain-dialogue-oracle';S=R/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',R/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
def a(op,**v):return dict(op=op,**v)
def field(name,value,**v):return a('field',field=name,value=value,**v)
def requests():
 rows=[]
 def add(actions,**v):rows.append(dict(actions=actions,**v))
 samples=['','a','a b','a  b ','<b>a b</b>','<b>aa bb cc dd</b>','<x=1>a b c d</x>','a<br>b','<a><b>aa bb cc dd</b></a>','<b>aa bb cc</long>','one two three four five six seven eight nine ten','\U0001f600 a \u03a3 b','<b>','</>',' ' * 8]
 for limit in [4,8,16,54]:
  for text in samples:
   if all(len(re.sub(r"<([A-Za-z0-9=/]+)>","",word).encode("utf-16-le"))//2<=limit for word in text.split(" ")):rows.append(dict(split=dict(text=text,limit=limit)))
 rng=random.Random(530210)
 for _ in range(180):
  limit=rng.choice([4,8,16,54]);text=' '.join(rng.choice(['a','bb','ccc','','<b>x</b>'])for _ in range(rng.randrange(30)));rows.append(dict(split=dict(text=text,limit=limit)))
 rows.append(dict(split=dict(text=[65,0xd800,32,0xdc00],limit=54)))
 rows.append(dict(split=dict(text='x'*55+' a b',limit=54)))
 for count in range(7):
  for interpreter in range(4):
   add([a('lines',interpreter=interpreter,lines=['line'+str(i)for i in range(count)])])
 for lines in [None,[None],['',None,'z'],['first','second','third','fourth']]:add([a('lines',lines=lines)])
 for text in ['','hello',"That doesn't look right. Please try again.","That number's a bit large, don't you think?","Something went wrong here. Try again.",'<b>aa bb cc dd ee ff gg hh</b>',None]:add([a('send',text=text)])
 for phase in ['interpreter','dispatch','text','manager','open','chatbox','id','hidden']:
  for at in range(1,7):add([a('lines',lines=['a','b','c'])],failures={phase:at})
 for phase in ['dispatch','text','manager','open','chatbox','id','hidden']:
  for name,value in [('dispatch',1),('manager',1),('chatbox',None),('id',999)]:add([a('lines',lines=['a','b'])],hooks=[dict(event=phase,actions=[field(name,value)])])
 add([a('lines',lines=['old','second'])],hooks=[dict(event='dispatch',actions=[a('line',index=0,value='new'),a('line',index=1,value='late')])])
 add([a('lines',lines=['a','b'])],hooks=[dict(event='text',actions=[a('line',index=1,value='changed')])])
 for name in ['dispatch','manager','interpreter']:add([field(name,None),a('send',text='hello')])
 for active in [None,1,False]:
  for queue in [[],[0],[None,1],[0,1]]:
   for veto in [False,True]:add([field('chatbox',1),field('plugin',0),field('active',active),field('veto',veto),a('queue',values=queue),a('option')])
 for phase in ['debug','interpreter','dialogue','manager','closeChatbox','actions','action','component','listener','plugin','pluginHandle','activeHandle']:
  for at in [1,2,3]:add([field('chatbox',1),field('veto',True),field('plugin',0),a('queue',values=[0,1]),a('option')],failures={phase:at})
 add([field('chatbox',1),field('veto',True),field('plugin',0),a('option')],hooks=[dict(event='listener',actions=[field('plugin',1)])])
 add([field('chatbox',1),a('queue',values=[0]),a('option')],hooks=[dict(event='action',actions=[field('lookup',1,manager=1),field('manager',1),field('plugin',1)])])
 add([a('option')],hooks=[dict(event='dialogue',actions=[field('interpreter',1),field('active',1,interpreter=1)])])
 add([a('lines',lines=['captured','second'])],hooks=[dict(event='dispatch',actions=[a('replaceArray',values=['replacement','array']),a('line',index=0,value='new')])])
 add([field('interpreter',None),a('send',text='<b>'+'aa '*40+'</long>')])
 add([field('chatbox',1),field('veto',True),field('plugin',0),field('listener',True),a('option')],failures={'plugin':1})
 add([field('active',1),a('option')],hooks=[dict(event='dialogue',actions=[field('interpreter',1)])])
 add([a('queue',interpreter=1,values=[0]),a('option')],hooks=[dict(event='closeChatbox',actions=[field('interpreter',1)])])
 add([a('queue',values=[None,1]),a('option'),a('option')])
 return rows
def generate(check):
 lock=json.loads(c.read(R/'upstream.lock.json'));assert c.run(['git','-C',R/'reference/server','rev-parse','HEAD']).stdout.decode().strip()==lock['server']['commit'];c.run(['git','-C',R/'reference/server','diff','--quiet','HEAD','--'])
 O.mkdir(exist_ok=True);classes=O/'classes';classes.mkdir(exist_ok=True);java=[];kt=[]
 def adapter(name,text):
  p=O/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(java if name.endswith('.java')else kt).append(p)
 p=S/'core/api/DialUtils.kt';c.read(p);kt.append(p)
 source=c.read(S/'core/game/dialogue/DialogueInterpreter.java');method=c.method(source,'public Component sendDialogue(String... messages)')
 adapter('oracle/Interpreter.java',c.read(H/'Interpreter.java').replace('/* ORIGINAL_SEND */',method))
 source=c.read(S/'core/api/ContentAPI.kt');method=c.method(source,'fun sendDialogue(player: Player, message: String)')
 source=c.read(S/'core/net/packet/PacketProcessor.kt');branch=c.method(source,'is Packet.ContinueOption ->').split('{',1)[1][:-1]
 adapter('oracle/Bridge.kt',c.read(H/'Bridge.kt').replace('/* ORIGINAL_CONTENT_SEND */',method).replace('/* ORIGINAL_CONTINUE */',branch))
 adapter('core/game/interaction/InterfaceListeners.kt','package core.game.interaction\nobject InterfaceListeners{@JvmStatic fun run(p:oracle.Oracle.Player,c:oracle.Oracle.Component,op:Int,child:Int,slot:Int,item:Int):Boolean=oracle.Oracle.run(p,c,op,child,slot,item)}')
 p=H/'Oracle.java';c.read(p);java.append(p)
 jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jlock=json.loads(c.read(R/'oracle/auth/json.lock.json'));jsonjar=R/'runs/interfaces-oracle/json-simple-1.1.1.jar';assert hashlib.sha256(jsonjar.read_bytes()).hexdigest()==jlock['sha256'];cp=os.pathsep.join(map(str,[classes,std,jsonjar]))
 c.run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*kt,*java]);c.run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 rows=requests();data=''.join(json.dumps(row,ensure_ascii=True)+'\n'for row in rows).encode();exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 p=subprocess.run(list(map(str,[exe,'-Xmx768m','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Oracle'])),cwd=R,input=data,capture_output=True,timeout=180);assert p.returncode==0,p.stderr.decode(errors='replace');out=[json.loads(line)for line in p.stdout.splitlines()];assert len(out)==len(rows)
 fixture=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in out).encode();c.read(pathlib.Path(__file__))
 initial=json.loads(c.read(H/'initial.lock.json'));assert hashlib.sha256(b''.join(fixture.splitlines(keepends=True)[:initial['caseCount']])).hexdigest()==initial['sha256']
 provenance=dict(format=1,initialBaseline=initial,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(fixture).hexdigest(),scope='Complete unchanged DialUtils.kt, exact selected DialogueInterpreter.sendDialogue, ContentAPI.sendDialogue and PacketProcessor.ContinueOption bodies; player/packet/manager/component/interpreter/queue/listener/plugin methods are explicit mutable recording services. Finite positive-limit splitting; not complete dialogue/content, original packet writers, owning-player or live UI proof.')
 folder=R/'fixtures/plain-dialogue';folder.mkdir(exist_ok=True)
 if check:assert(folder/'oracle.jsonl').read_bytes()==fixture and json.loads((folder/'provenance.json').read_text(encoding='utf-8'))==provenance
 else:(folder/'oracle.jsonl').write_bytes(fixture);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
 print(len(rows),'original plain dialogue/Continue scenarios reproduced'if check else'original plain dialogue/Continue scenarios generated',flush=True)
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);v=p.parse_args();c.JAVA_HOME=v.java_home;generate(v.check)
