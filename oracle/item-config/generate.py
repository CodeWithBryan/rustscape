"""Compile complete original item configuration loader and constructor value classes."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,re,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=pathlib.Path(os.environ.get('RS09_ITEM_CONFIG_ORACLE_RUN',ROOT/'runs/item-config-oracle'));SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');compiler=importlib.util.module_from_spec(spec);spec.loader.exec_module(compiler)
read,run=compiler.read,compiler.run
def requests():
    cases=[];rng=random.Random(530)
    def add(texts,**kwargs):cases.append(dict(kind='load',steps=[dict(text=text)if isinstance(text,str)else text for text in texts],**kwargs))
    def encoded(rows):return json.dumps(rows,ensure_ascii=True,separators=(',',':'))
    for root in [[],None,{},1,True,'abc',[None],[True],[1],[[]]]:add([encoded(root)])
    groups={
      'int':['equip_audio','point_price','tokkul_price','archery_ticket_price','castle_wars_ticket_price','ge_buy_limit','weapon_interface','attack_speed','equipment_slot','render_anim','turn90ccw_anim','turn90cw_anim','turn180_anim','run_anim','walk_anim','stand_anim','low_alchemy','high_alchemy','grand_exchange_price','id'],
      'bool':['fun_weapon','rare_item','bankable','two_handed','has_special','remove_sleeves','remove_beard','remove_head','hat','destroy','lendable','tradeable','alchemizable'],
      'arrays':['absorb','bonuses','attack_anims','attack_audios'],
      'special':['requirements','defence_anim','weight','examine','destroy_message','name','stand_turn_anim','shop_price','unknown']}
    for group,keys in groups.items():
        values={'int':[None,'','null',-2**31,2**31-1,'2147483648','1.0','-0','+2','१२',True,[],{}],
          'bool':[None,'','null',True,False,'TRUE',' True','1',1,[],{}],
          'arrays':[None,'','null','1','1,2,-3','1,','1,,2','1, 2','2147483648','१२,3',[],[1,2]],
          'special':[None,'','null','1','true','1,2','{1,2}-{3,4}','1,2-1,9','-1,3','NaN','Infinity',1.5,[],{'b':1,'a':2},['/','€','Ω','😀']]}
        for key in keys:
            for value in values[group]:
                row=dict(id='1',tradeable='true',high_alchemy='123',name='configured',weight='1.5');row[key]=value
                add([encoded([row])])
    for spelling in ['0x1.8p2','-0.0','1e309','1e-999','0x1p-1075',' 2.5 ','2.5f','nan','NaN','-Infinity']:
        add([encoded([dict(id=1,weight=spelling)])])
    for raw in ['[]','[,,,]','[{}{}]','[{"id""1" "weight"::"2.5"}]','[{"id":01,"weight":1e999}]','[{"id":1,"weight":-1e999}]','[{"id":1,"examine":"line\nbreak"}]','[{"id":1,}]','[{"id":1},]','[{"id":1 "name":"A"}]','[{"id":1,,"name":"A"}]',
      '', '[','{','[{}','[1 2]','[truefalse]','[nulltrue]','{"a"}', '[{"id":9223372036854775808}]','[+1]','[.1]','[1.]','[1e]','["\\z"]','["\\uZZZZ"]','["unterminated','["\\uD83D\\uDE00"]','[] trailing','\ufeff[]','[\v]']:
        add([raw])
    good=encoded([dict(id=1,requirements='{0,20}-{16,30}-{64,40}',attack_anims='1,2',attack_audios='3,4',bonuses='5,6',defence_anim='9',name='new')])
    bad=encoded([dict(id=1,requirements='{3,4}-bad',weight='2')])
    add([good,dict(capture=['requirements','bonuses','attack_anims','attack_audios','defence_anim']),bad,dict(mutate=0,key=64,value=99),good,dict(mutate=0,key=128,value=20),dict(missing=True),encoded([])])
    for ids in [[1,1],[99,1],[2,2],[1,2],[0,1,99]]:add([encoded([dict(id=id,requirements='1,2',bankable='false')for id in ids])])
    add([good,encoded([dict(id=1,name='last')])],logFail=True)
    for _ in range(80):
        names=list(sum(groups.values(),[]));rng.shuffle(names);row={'id':'1'}
        for key in names[:rng.randrange(1,20)]:row[key]=rng.choice(['true','1','1,2','{1,2}-{3,4}','null','',None,'bad'])
        row['id']='1';add([encoded([dict(id=0,weight='0.0'),row,dict(id=99,name='last')])])
    cases.append(dict(kind='constants'))
    for raw in ['null','true','false','0','-0','01','-01','1.0','-0.0','1e309','-1e309','1e-999','9223372036854775807','-9223372036854775808','9223372036854775808','[,,]','[truefalse null0]','{"a"::1,,"b"2,"a":3}',
      '"\\z"','"\\uZZZZ"','"\\u1"','"\\x\\b"','"line\nbreak"','"\\uD83D\\uDE00"','["/","€","Ω","😀",1e309]','{"BB":1,"Aa":2,"AaAa":3,"BBBB":4}',*['[{"id":1,"name":"\\'+c+'"}]'for c in 'abcdefghijklmnopqrstuvwxyz0123456789']]:cases.append(dict(kind='json',text=raw))
    for _ in range(150):
        value=rng.choice([rng.randrange(-2**63,2**63),rng.uniform(-1e100,1e100),'/€Ω😀\n\t\u0080\u2001',None,True,False,[rng.random(),False,None],{'Aa':1,'BB':2,'1':3}]);raw=json.dumps(value,ensure_ascii=True,separators=(',',':'));cases.append(dict(kind='json',text=raw))
        if len(raw)>3:cases.append(dict(kind='json',text=raw[:rng.randrange(1,len(raw))]))
    return cases

def generate(check,audit_file=None):
    lock=json.loads(read(ROOT/'upstream.lock.json'))
    for part in ['server','client']:
        path=ROOT/'reference'/part
        if run(['git','-C',path,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
        run(['git','-C',path,'diff','--quiet','HEAD','--'])
    RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);files=RUN/'files';files.mkdir(exist_ok=True)
    jars=compiler.artifacts();artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'));jar=RUN/'json-simple-1.1.1.jar'
    if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as response:data=response.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    stdlib=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));cp=os.pathsep.join(map(str,[classes,stdlib,jar]));java=[]
    def adapter(name,text):
        path=RUN/'adapters'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf-8');java.append(path)
    item=read(SERVER/'core/cache/def/impl/ItemDefinition.java');fields=item[item.index('private static final Map<Integer, ItemDefinition> DEFINITIONS'):item.index('public ItemDefinition()')]
    methods=''.join(compiler.method(item,m)for m in ['public ItemDefinition()','public static ItemDefinition forId(','public static Map<Integer, ItemDefinition> getDefinitions()'])
    adapter('core/cache/def/impl/ItemDefinition.java','package core.cache.def.impl;import java.util.*;import core.game.node.item.Item;import core.game.interaction.OptionHandler;public class ItemDefinition extends core.cache.def.Definition<Item>{'+fields+methods+'}')
    animator=read(SERVER/'core/game/node/entity/impl/Animator.java')
    adapters={
      'core/game/node/Node.java':'package core.game.node;public class Node {}',
      'core/game/node/item/Item.java':'package core.game.node.item;public class Item extends core.game.node.Node {}',
      'core/game/interaction/OptionHandler.java':'package core.game.interaction;public class OptionHandler {}',
      'core/game/node/entity/impl/Animator.java':'package core.game.node.entity.impl;public class Animator {'+compiler.method(animator,'public static enum Priority')+'}',
      'core/cache/def/impl/AnimationDefinition.java':'package core.cache.def.impl;public class AnimationDefinition {public static AnimationDefinition forId(int id){throw new UnsupportedOperationException("Animation definitions outside constructor oracle");}public int getDurationTicks(){throw new UnsupportedOperationException("Animation duration outside constructor oracle");}}',
      'core/game/node/scenery/Scenery.java':'package core.game.node.scenery;public class Scenery {}',
      'core/ServerConstants.java':'package core;public class ServerConstants {public static String CONFIG_PATH;}',
      'core/tools/Log.java':'package core.tools;public class Log {public static int FINE=1;}',
      'core/tools/SystemLogger.java':'package core.tools;public class SystemLogger {}',
      'core/tools/StringUtils.java':'package core.tools;public class StringUtils {public static boolean isPlusN(String name){throw new UnsupportedOperationException("Examine generation outside oracle");}}',
      'oracle/Events.java':'package oracle;public class Events {public static java.util.List<String> logs=new java.util.ArrayList<>();public static boolean fail;public static void log(String text){logs.add(text);if(fail)throw new IllegalStateException("injected log failure");}}',
      'core/api/ContentAPIKt.java':'package core.api;public class ContentAPIKt {public static void log(Object source,int level,String text){oracle.Events.log(text);}}',
    }
    for name,text in adapters.items():adapter(name,text)
    for name in ['core/cache/def/Definition.java','core/game/world/update/flag/context/Animation.java']:path=SERVER/name;read(path);java.append(path)
    run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
    kotlin=[]
    for name in ['core/game/node/entity/player/link/audio/Audio.kt','core/game/system/config/ItemConfigParser.kt']:path=SERVER/name;read(path);kotlin.append(path)
    logging=RUN/'Logging.kt';logging.write_text('@file:JvmName("ContentAPIKt")\npackage core.api\nfun log(source:Any?,level:Int,text:String){oracle.Events.log(text)}\n',encoding='utf-8')
    run(['java','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*kotlin,logging,RUN/'adapters/core/game/node/entity/impl/Animator.java',SERVER/'core/game/world/update/flag/context/Animation.java'])
    read(HERE/'ItemConfigOracle.java');run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,HERE/'ItemConfigOracle.java'])
    command=['java','-Xmx2g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.ItemConfigOracle',str(files)]
    if compiler.JAVA_HOME:command[0]=str(compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java'))
    def evaluate(cases):
        file=RUN/'input.jsonl';file.write_bytes(''.join(json.dumps(case,sort_keys=True,ensure_ascii=True)+'\n'for case in cases).encode())
        with file.open('rb')as stdin:result=subprocess.run(command,cwd=ROOT,stdin=stdin,capture_output=True,timeout=120)
        if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
        output=[json.loads(line)for line in result.stdout.splitlines()if line]
        if len(output)!=len(cases):raise RuntimeError('Missing item config cases')
        return output
    output=evaluate(requests());data=''.join(json.dumps(case,sort_keys=True,ensure_ascii=True,separators=(',',':'))+'\n'for case in output).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py');read(ROOT/'tools/port_item_config.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unmodified ItemConfigParser.kt, Audio.kt, Animation.java and Definition.java; original priority enum and item fields/constructor/lookup. Actual JSON-simple 1.1.1 DOM parser, actual isolated UTF-8 FileReader loads. Controlled logging, metadata registry and throwing animation-definition/scenery/world adapters. Typed values, requirements iteration/partial writes, retained object identity, reload and parse failures. No animation execution, general Node/world, complete item definition/content binding or complete player profile claim.')
    folder=ROOT/'fixtures/item-config'
    if check:
        if (folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text())!=provenance:raise RuntimeError('Item config oracle/provenance differs; review before regeneration')
        print(f'{len(output)} original item-configuration cases reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(output)} original item-configuration cases.')
    if audit_file:
        observed=evaluate([json.loads(audit_file.read_text(encoding='utf-8'))])[0];(RUN/'audit-output.json').write_text(json.dumps(observed,sort_keys=True,separators=(',',':'))+'\n',encoding='utf-8');print('Wrote complete pinned item-configuration observations.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);p.add_argument('--audit-file',type=pathlib.Path);a=p.parse_args();compiler.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check,a.audit_file)
