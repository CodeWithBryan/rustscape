"""Compare original core profile methods integrated with actual containers and timers."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,re,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/core-profile-oracle';SERVER=ROOT/'reference/server/Server/src/main'
def load(name,path):
    spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
timers=load('timers',ROOT/'oracle/timers/generate.py');inventory=load('inventory',ROOT/'oracle/inventory/generate.py');compiler=timers.compiler;read,run=compiler.read,compiler.run
def requests():
    cases=[];rng=random.Random(530)
    def op(name,**args):return dict(op=name,**args)
    def row(slot=0,id=1,amount=3,charge=1000):return {k:str(v)for k,v in locals().items()}
    def core(**extra):return dict(inventory=[row()],bank=[row(2,2,99)],equipment=[row(3,3)],location='3200,3201,0',**extra)
    def load(value):return op('coreLoad',text=json.dumps(value,separators=(',',':')))
    def add(actions,**kw):cases.append(dict(timers={'p':dict(id='persist',interval=5,**{'class':'PersistentTimer'}),'q':dict(id='other',interval=2,**{'class':'ChildPersistent'})},registry={'persist':'p','other':'q'},actions=actions,**kw))
    base=core(bankSecondary=[row(4,4,7)],bankTabs=[dict(index='2',startSlot='14')],bankTabsSecondary=[dict(index='10',startSlot='-1')],useSecondaryBank=True,varp=[dict(index='64',value='3'),None,dict(index='0',value='7')],timers={'persist':{'ticksLeft':'9'}})
    add([load(dict(core_data=base)),op('coreSave'),op('process'),op('coreSave'),load(dict(core_data=core())),op('coreSave')])
    for value in [None,{},dict(core_data=None),dict(core_data=[]),dict(core_data={})]:add([load(dict(core_data=base)),load(value),op('coreSave')])
    for key in ['inventory','bank','bankSecondary','equipment','location','bankTabs','bankTabsSecondary','useSecondaryBank','varp','timers']:
        for value in [None,True,False,0,1.5,'x',{},[],[None],[False]]:
            v=core();v[key]=value
            add([load(dict(core_data=base)),load(dict(core_data=v)),op('coreSave')])
        v=core();v.pop(key,None);add([load(dict(core_data=base)),load(dict(core_data=v)),op('coreSave')])
    for key in ['inventory','bank','bankSecondary','equipment']:
        for field,bad in [('slot','-1'),('slot','496'),('id','9'),('id','-1'),('id','5614'),('amount','bad'),('amount',None),('charge','65536'),('id','65537')]:
            v=core();badrow=row(1);badrow[field]=bad;v[key]=[row(0),badrow,row(2)]
            add([load(dict(core_data=base)),load(dict(core_data=v)),op('coreSave')])
    for key in ['bankTabs','bankTabsSecondary']:
        for index in ['-1','0','10','11','2147483648','bad',None]:
            for start in ['-4','bad',None]:
                v=core();v[key]=[dict(index='1',startSlot='7'),dict(index=index,startSlot=start),dict(index='3',startSlot='9')]
                add([load(dict(core_data=base)),load(dict(core_data=v)),op('coreSave')])
    for location in ['1','1,2','1,2,3','1,2,3,4','1,2,3,bad','1,2,3,',' 1,2,3','-16385,16384,-5','2147483647,-2147483648,-2147483648','१२,३,४']:
        v=core();v['location']=location;add([load(dict(core_data=base)),load(dict(core_data=v)),op('coreSave')])
    for raw in [None,{},dict(index=None,value='bad'),dict(index='bad'),dict(index='2'),dict(index='2',value=None),dict(index='2',value='bad'),dict(index='2',value=True),dict(index='2',value='2147483648'),dict(index='2',value='-7')]:
        v=core(varp=[dict(index='8',value='12'),raw,dict(index='9',value='13')]);add([load(dict(core_data=base)),load(dict(core_data=v)),op('coreSave')])
    for stage in ['before','register','initial','run','remove','save','parse']:
        for callback in [op('fail'),op('coreVarp',index=32,value=99,save=True),op('coreLocation',value=[1,2,3]),op('coreSecondary',value=True)]:
            label='created0'if stage in ['before','register','parse']else'p'
            add([op('root',text='{"core_data":"previous","sentinel":7}'),load(dict(core_data=base)),op('register',timer='p'),op('process'),op('tick',value=20),op('process'),op('coreSave'),op('remove',timer='p'),op('coreSave')],scripts=[dict(timer=label,event=stage,actions=[callback])])
    add([op('root',text='{"core_data":"previous"}'),op('coreLocation',value=None),op('coreSave')])
    add([op('coreRaw',slot=0,id=65537,amount=-9,charge=65536,index=-1),op('coreRaw',container='bank',slot=495,id=2,index=17),op('coreSave'),load(dict(core_data=core())),op('coreSave')])
    for field in ['slot','id','amount','charge']:
        value=core();value['inventory']=[row(0),dict(row(1),**{field:'NONFINITE'})]
        encoded=json.dumps(dict(core_data=value),separators=(',',':')).replace('"NONFINITE"','1e999')
        add([load(dict(core_data=base)),op('coreLoad',text=encoded),op('coreSave')])
    value=core(varp=[dict(index='1',value='7'),dict(index='2',value='NONFINITE')])
    add([load(dict(core_data=base)),op('coreLoad',text=json.dumps(dict(core_data=value),separators=(',',':')).replace('"NONFINITE"','-1e999')),op('coreSave')])
    for keys in [[i*64 for i in range(40)],list(reversed([i*64 for i in range(40)])),[-2**31,-1,0,1,2**31-1]]:
        add([op('coreVarp',index=k,value=i,save=rng.choice([None,False,True]))for i,k in enumerate(keys)]+[op('coreSave')])
    for _ in range(50):
        v=core(varp=[dict(index=str(rng.randrange(-100,100)),value=str(rng.randrange(-1000,1000)))for _ in range(12)],timers={'persist':{'ticksLeft':str(rng.randrange(-100,100))}},bankTabs=[dict(index=str(rng.randrange(11)),startSlot=str(rng.randrange(-100,500)))for _ in range(6)])
        for k,size in [('inventory',28),('bank',496),('bankSecondary',496),('equipment',14)]:v[k]=[row(rng.randrange(size),rng.randrange(8),rng.randrange(-100,100),rng.randrange(-1000,100000))for _ in range(8)]
        add([load(dict(core_data=v)),op('coreSave'),op('process'),op('coreSave')],tick=rng.randrange(-2**31,2**31),step=rng.choice([0,1]))
    return cases

def generate(check):
    timers.compiler.JAVA_HOME=compiler.JAVA_HOME;inventory.compiler.JAVA_HOME=compiler.JAVA_HOME
    timers.generate(True);inventory.generate(True);compiler.SOURCES.update(inventory.compiler.SOURCES)
    RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    jars=compiler.artifacts();jar=timers.RUN/'json-simple-1.1.1.jar';runtime=[p for p in jars if p.name.startswith(('kotlin-stdlib-','kotlin-reflect-'))]
    cp=os.pathsep.join(map(str,[classes,timers.RUN/'classes',inventory.RUN/'classes',jar,*runtime]));compiler_cp=os.pathsep.join(map(str,jars))
    def compile_kt(inputs):run(['java','-Xmx512m','-cp',compiler_cp,'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*inputs])
    def adapter(name,text):
        path=RUN/'adapters'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf-8');return path
    location=read(SERVER/'core/game/world/map/Location.kt');nested=compiler.method(location,'value class LocationHash(')
    loc=adapter('Location.kt','package core.game.world.map\nclass Location(x:Int,y:Int,z:Int=0){var hash=LocationHash(x,y,(z%4+4)%4);val x get()=hash.x;val y get()=hash.y;val z get()=hash.z;\n@JvmInline '+nested+'}')
    utils=read(SERVER/'core/JSONUtils.kt');util=adapter('JSONUtils.kt','package core\nimport core.game.world.map.Location\nclass JSONUtils {companion object{'+compiler.method(utils,'fun parseLocation(locString: String)')+'}}')
    compile_kt([loc,util])
    bank_source=read(SERVER/'core/game/container/impl/BankContainer.java');equipment_source=read(SERVER/'core/game/container/impl/EquipmentContainer.java');player_source=read(SERVER/'core/game/node/entity/player/Player.java');constants_source=read(SERVER/'core/ServerConstants.kt')
    bank_fields=bank_source[bank_source.index('public static final int SIZE'):bank_source.index('public BankContainer(Player player)')]
    bank=adapter('core/game/container/impl/BankContainer.java','package core.game.container.impl;import core.game.container.*;import core.game.node.entity.player.Player;import core.ServerConstants;public class BankContainer extends Container {'+bank_fields+compiler.method(bank_source,'public BankContainer(Player player)')+compiler.method(bank_source,'public int[] getTabStartSlot()')+'}')
    eq=adapter('core/game/container/impl/EquipmentContainer.java','package core.game.container.impl;import core.game.container.*;import core.game.node.entity.player.Player;public class EquipmentContainer extends Container {private final Player player;'+compiler.method(equipment_source,'public EquipmentContainer(Player player)')+'}')
    listener=adapter('core/game/container/impl/RecordingListener.java','package core.game.container.impl;import core.game.container.*;public class RecordingListener implements ContainerListener{public void update(Container c,ContainerEvent e){throw new AssertionError("Profile should not fire container updates");}public void refresh(Container c){throw new AssertionError("Profile should not refresh containers");}}')
    player_fields='\n'.join(re.search(r'[^\n]*\b'+name+r' = [^;]+;',player_source).group(0)for name in ['inventory','equipment','bank','bankSecondary','useSecondaryBank'])
    player_getters=''.join(compiler.method(player_source,signature)for signature in ['public Container getInventory()','public EquipmentContainer getEquipment()','public BankContainer getBankPrimary()','public BankContainer getBankSecondary()','public BankContainer getBank()'])
    player=adapter('core/game/node/entity/player/Player.java','package core.game.node.entity.player;import java.util.*;import core.game.container.*;import core.game.container.impl.*;import core.game.world.map.Location;public class Player extends core.game.node.entity.Entity {'+player_fields+player_getters+'private Location location=new Location(3222,3218,0);private final HashMap<Integer,Integer> varpMap=new HashMap<>();private final HashMap<Integer,Boolean> saveVarp=new HashMap<>();public Location getLocation(){return location;}public void setLocation(Location v){location=v;}public HashMap<Integer,Integer> getVarpMap(){return varpMap;}public HashMap<Integer,Boolean> getSaveVarp(){return saveVarp;}public boolean hasModalOpen(){oracle.Model.INSTANCE.getEvents().add(Arrays.asList("modal",oracle.Model.INSTANCE.getModal()));return oracle.Model.INSTANCE.getModal();}}')
    java=[bank,eq,listener,player,adapter('core/ServerConstants.java','package core;public class ServerConstants {public static int BANK_SIZE='+re.search(r'var BANK_SIZE: Int = (\d+)',constants_source).group(1)+';}')]
    for name in ['BankListener','EquipmentListener','InventoryListener']:java.append(adapter('core/game/container/impl/'+name+'.java','package core.game.container.impl;public class '+name+' extends RecordingListener{public '+name+'(core.game.node.entity.player.Player player){}}'))
    run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
    parser=read(SERVER/'core/game/node/entity/player/info/login/PlayerSaveParser.kt');saver=read(SERVER/'core/game/node/entity/player/info/login/PlayerSaver.kt')
    parse=adapter('Parser.kt','package core.game.node.entity.player.info.login\nimport core.game.node.entity.player.Player\nimport org.json.simple.*\nimport core.JSONUtils\nclass PlayerSaveParser(val player:Player,var saveFile:JSONObject?){'+compiler.method(parser,'fun parseCore()')+'}')
    save=adapter('Saver.kt','package core.game.node.entity.player.info.login\nimport core.game.node.entity.player.Player\nimport org.json.simple.*\nimport core.game.container.Container\nclass PlayerSaver(val player:Player){'+compiler.method(saver,'fun saveContainer(container: Container)')+compiler.method(saver,'fun saveCoreData(root: JSONObject)')+'}')
    # Record source for constructor dimensions/selection; the adapters do not run concrete listeners.
    for path in ['core/game/node/entity/player/Player.java','core/game/container/impl/BankContainer.java','core/game/container/impl/EquipmentContainer.java','core/ServerConstants.kt']:read(SERVER/path)
    read(HERE/'Support.kt');compile_kt([parse,save,HERE/'Support.kt'])
    cases=requests();file=RUN/'input.jsonl';file.write_bytes(''.join(json.dumps(c,sort_keys=True)+'\n'for c in cases).encode())
    executable=compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if compiler.JAVA_HOME else'java'
    with file.open('rb')as stdin:result=subprocess.run(list(map(str,[executable,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.SupportKt'])),cwd=ROOT,stdin=stdin,capture_output=True,timeout=120)
    if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
    output=[json.loads(line)for line in result.stdout.splitlines()if line]
    if len(output)!=len(cases):raise RuntimeError('Missing core profile cases')
    data=''.join(json.dumps(c,sort_keys=True,ensure_ascii=True,separators=(',',':'))+'\n'for c in output).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=json.loads(read(ROOT/'upstream.lock.json')),sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Verbatim PlayerSaveParser.parseCore, PlayerSaver.saveContainer/saveCoreData, JSONUtils.parseLocation and original packed LocationHash; complete original containers/items/timer classes reused. Explicit Player/bank/equipment constructors, fail-on-use listener adapters, controlled definition/registry/content/clock bindings. Core sections and partial side effects, not complete Player construction, other profile sections, file I/O, live bank/equipment actions, scheduler or world entry.')
    folder=ROOT/'fixtures/core-profile'
    if check:
        if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Core profile oracle/provenance differs; review before regeneration')
        print(f'{len(output)} original core profile cases reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(output)} original core profile cases.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();compiler.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
