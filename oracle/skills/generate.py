"""Compile unchanged Skills and compare explicit entity/content boundary traces."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/skills-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');compiler=importlib.util.module_from_spec(spec);spec.loader.exec_module(compiler)
read=compiler.read;run=compiler.run
ADAPTERS={
'core/ServerConstants.java':'package core;public class ServerConstants {public static int CURRENT_SAVEFILE_VERSION=6;}',
'core/game/world/GameWorld.java':'package core.game.world;public class GameWorld {public static int getTicks(){oracle.Model.call("tick",oracle.Model.ticks);return oracle.Model.ticks;}}',
'core/api/ContentAPIKt.java':'package core.api;public class ContentAPIKt {public static int getWorldTicks(){oracle.Model.call("api_tick",oracle.Model.ticks);return oracle.Model.ticks;}}',
'core/game/node/entity/Entity.java':'''package core.game.node.entity;
public class Entity {public core.game.node.entity.skill.Skills skills;public Properties properties=new Properties();public core.game.node.entity.player.link.request.assist.AssistSession assist;
 public Properties getProperties(){return properties;}public <T>T getExtension(Class<T> type){return type.cast(assist);}public core.game.node.entity.player.Player asPlayer(){return (core.game.node.entity.player.Player)this;}public void dispatch(Object event){if(event instanceof core.game.event.XPGainEvent){core.game.event.XPGainEvent e=(core.game.event.XPGainEvent)event;oracle.Model.call("xp_event",e.skill,oracle.Model.bits(e.amount));oracle.Model.mutate(skills,"event");}else{core.game.event.DynamicSkillLevelChangeEvent e=(core.game.event.DynamicSkillLevelChangeEvent)event;oracle.Model.call("dynamic",e.skill,e.previous,e.level);oracle.Model.mutate(skills,"dynamic");}}
 public core.game.node.entity.skill.Skills getSkills(){return skills;}
 public static class Properties {public int combat;public int getCombatLevel(){return combat;}public void setCombatLevel(int value){oracle.Model.call("combat",value);combat=value;}}
}''',
'core/game/node/entity/player/Player.java':'''package core.game.node.entity.player;
public class Player extends core.game.node.entity.Entity {public int version=6;public boolean assister;public Equipment equipment=new Equipment();public Familiar familiar=new Familiar();public java.util.Map<String,Object> attributes=new java.util.HashMap<>();public Equipment getEquipment(){return equipment;}public Familiar getFamiliarManager(){return familiar;}public boolean isArtificial(){oracle.Model.call("artificial");return oracle.Model.flag("artificial");}public String getUsername(){oracle.Model.call("username");return "player";}public void updateAppearance(){oracle.Model.call("appearance");oracle.Model.mutate(skills,"appearance");}public java.util.Map<String,Object> getAttributes(){return attributes;}public <T>T getAttribute(String key,T value){oracle.Model.call("tutorial");return (T)Boolean.valueOf(oracle.Model.flag("tutorial"));}
 public static class Equipment {public boolean containsItem(core.game.node.item.Item item){oracle.Model.call("equipment",item.getId());return oracle.Model.equipment.contains(item.getId());}}
 public static class Familiar {public int combat;public int getSummoningCombatLevel(){return combat;}public void setSummoningCombatLevel(int level){oracle.Model.call("summon",level);combat=level;}}
}''',
'core/game/node/entity/npc/NPC.java':'package core.game.node.entity.npc;public class NPC extends core.game.node.entity.Entity {public Definition getDefinition(){return new Definition();}public static class Definition {public int getCombatLevel(){oracle.Model.call("npc_combat");return oracle.Model.integer("npcCombat",42);}}}',
'core/game/node/entity/combat/ImpactHandler.java':'package core.game.node.entity.combat;public class ImpactHandler {}',
'core/game/node/item/Item.java':'package core.game.node.item;public class Item {private int id;public Item(int id){this.id=id;}public int getId(){return id;}}',
'core/cache/def/impl/ItemDefinition.java':'package core.cache.def.impl;public class ItemDefinition {public static ItemDefinition forId(int id){return new ItemDefinition();}public String getName(){return "adapter";}}',
'core/game/event/XPGainEvent.java':'package core.game.event;public class XPGainEvent {public int skill;public double amount;public XPGainEvent(int s,double a){skill=s;amount=a;}}',
'core/game/event/DynamicSkillLevelChangeEvent.java':'package core.game.event;public class DynamicSkillLevelChangeEvent {public int skill,previous,level;public DynamicSkillLevelChangeEvent(int s,int p,int l){skill=s;previous=p;level=l;}}',
'core/game/node/entity/player/info/PlayerMonitor.java':'package core.game.node.entity.player.info;public class PlayerMonitor {public static void logXpGains(core.game.node.entity.player.Player p,java.util.ArrayList<kotlin.Pair<Integer,Double>> diffs){java.util.List<Object> out=new java.util.ArrayList<>();for(kotlin.Pair<Integer,Double> pair:diffs)out.add(java.util.Arrays.asList(pair.getFirst(),oracle.Model.bits(pair.getSecond())));oracle.Model.call("xp_log",out);oracle.Model.mutate(p.getSkills(),"log");}}',
'core/game/world/repository/Repository.java':'package core.game.world.repository;public class Repository {public static void sendNews(String text){oracle.Model.call("news",text);oracle.Model.mutate(oracle.Model.owner.skills,"news");}}',
'core/net/packet/context/SkillContext.java':'package core.net.packet.context;public class SkillContext {public core.game.node.entity.player.Player player;public int skill;public SkillContext(core.game.node.entity.player.Player p,int s){player=p;skill=s;}}',
'core/net/packet/out/SkillLevel.java':'package core.net.packet.out;public class SkillLevel {}',
'core/net/packet/PacketRepository.java':'package core.net.packet;public class PacketRepository {public static void send(Class<?> type,core.net.packet.context.SkillContext context){core.game.node.entity.skill.Skills s=context.player.skills;int i=context.skill;oracle.Model.call("packet",i,s.getStaticLevel(i),s.getDynamicLevels()[i],oracle.Model.bits(s.getExperience(i)),s.getLifepoints(),oracle.Model.bits(s.getPrayerPoints()));oracle.Model.mutate(s,"packet");}}',
'core/plugin/CorePluginTypes/XPGainPlugins.java':'package core.plugin.CorePluginTypes;public class XPGainPlugins {public static void run(core.game.node.entity.player.Player p,int slot,double amount){oracle.Model.call("xp_plugins",slot,oracle.Model.bits(amount));oracle.Model.mutate(p.skills,"plugin");}}',
'core/game/node/entity/skill/LevelUp.java':'package core.game.node.entity.skill;public class LevelUp {public static void levelup(core.game.node.entity.player.Player p,int slot,int amount){oracle.Model.call("level_up",slot,amount);oracle.Model.mutate(p.skills,"level");}public static void sendFlashingIcons(core.game.node.entity.player.Player p,int skill){oracle.Model.call("flash",skill);}}',
'content/global/skill/skillcapeperks/SkillcapePerks.java':'package content.global.skill.skillcapeperks;public class SkillcapePerks {public static int DAMAGE_SPONG=1;public static boolean isActive(int perk,core.game.node.entity.player.Player p){oracle.Model.call("perk");return oracle.Model.flag("perk");}}',
'org/rs09/consts/Items.java':'package org.rs09.consts;public class Items {public static int FLAME_GLOVES_13660=13660,RING_OF_FIRE_13659=13659;}',
'content/global/handlers/item/equipment/brawling_gloves/BrawlingGlovesManager.java':'''package content.global.handlers.item.equipment.brawling_gloves;
public class BrawlingGlovesManager {public java.util.Map<Integer,Integer> GloveCharges=new java.util.HashMap<>();public static BrawlingGlovesManager getInstance(core.game.node.entity.player.Player p){oracle.Model.call("brawling");if(p==null)throw new NullPointerException();BrawlingGlovesManager result=new BrawlingGlovesManager();if(oracle.Model.flag("gloves"))result.GloveCharges.put(1,1);return result;}public double getExperienceBonus(){oracle.Model.call("bonus");return oracle.Model.number("bonus",0.5);}public void updateCharges(int id,int count){oracle.Model.call("consume",id,count);if(oracle.Model.flag("removeGlove"))oracle.Model.equipment.remove(id);oracle.Model.mutate(oracle.Model.owner.skills,"consume");}}
''',
}
def requests():
    cases=[]
    def action(op,**kwargs):return dict(op=op,**kwargs)
    def add(actions,**config):cases.append(dict(kind='sequence',config=config,actions=actions))
    for entity in ['player','npc','other']:
        add([action('configure'),action('refresh'),action('restore'),action('pulse')],entity=entity)
        for skill in [-1,0,3,5,23,24]:
            for value in [-2**31,-1,0,1,99,2**31-1]:
                add([action('setLevel',skill=skill,value=value),action('getLevel',skill=skill),action('staticLevel',skill=skill,value=min(value,10000)),action('combat')],entity=entity)
        for value in [-2**31,-1,0,1,10,11,2**31-1]:
            add([action('life',value=value),action('maxLife'),action('heal',value=20),action('hit',value=value),action('healRaw',value=value)],entity=entity,perk=True)
        for text in ['-Infinity','-100','-0.0','0','1','1.5','100','Infinity','NaN']:
            add([action('prayerSet',value=text),action('prayerInc',value=text),action('prayerDec',value=text),action('recharge')],entity=entity)
    for skill in [0,3,5,23]:
        for amount in [-2**31,-1,0,1,100,2**31-1]:
            for maximum in [-1,0,1,20]:add([action('update',skill=skill,amount=amount,maximum=maximum),action('updateDefault',skill=skill,amount=amount),action('drain',skill=skill,drain='0.3',maximum='0.8')])
    for slot in range(24):
        for text in ['1','83','1000','200000001','-100','NaN','Infinity']:
            add([action('xp',skill=slot,value=text),action('save')],tutorial=True)
        add([action('xp',skill=slot,value='1000'),action('save')],tutorial=False)
    for tutorial in [False,True]:
        for artificial in [False,True]:
            add([action('rawXp',skill=0,value='199999999'),action('xp',skill=0,value='2'),action('xp',skill=0,value='10')],tutorial=tutorial,artificial=artificial)
    for entity in ['npc','other']:add([action('xp',skill=0,value='1')],entity=entity)
    for equipment in [[],[13659],[13660],[13659,13660],[13845],[13845,13660,13659],[13851,13660,13659]]:
        for slot in [0,1,2,7,11]:
            add([action('xp',skill=slot,value='83'),action('xp',skill=slot,value='83')],equipment=equipment,gloves=True,bonus=3.0,tutorial=True,removeGlove=True)
    for translate in [False,True]:
        for restricted in [False,True]:
            for own in [False,True]:
                add([action('getLevel',skill=12),action('getLevelDiscard',skill=12),action('xp',skill=12,value='500')],assist=True,translate=translate,restricted=restricted,assistOwn=own,assistEnabled=True,assistLevel=80,tutorial=True)
    for callback in ['plugin','level','event','packet','consume','appearance','news','log']:
        add([action('tick',value=200),action('xp',skill=0,value='1000'),action('xp',skill=1,value='1')],tutorial=True,gloves=True,equipment=[13845],mutations={callback:dict(skill=6,xp='2000',tick=400)})
    for failure in ['packet','dynamic','combat','summon','perk','brawling','equipment','bonus','consume','tutorial','artificial','username','news','xp_plugins','appearance','level_up','xp_event','xp_log','flash','npc_combat']:
        add([action('fail',value=failure),action('tick',value=200),action('xp',skill=0,value='200000001'),action('setLevel',skill=5,value=10),action('maxLife'),action('refresh'),action('combat')],tutorial=False,gloves=True,equipment=[13845],entity='npc'if failure=='npc_combat'else'player')
    for tick in [-2**31,0,199,200,201,2**31-1]:
        add([action('tick',value=tick),action('xp',skill=3,value='1'),action('tick',value=(tick+200+2**31)%2**32-2**31),action('xp',skill=6,value='50')],tutorial=True)
    for version in [0,2,3,6]:
        rows=[dict(id=str(i),dynamic=str(2+i),static=str(5+i),experience='83')for i in range(24)]
        for order in [rows,list(reversed(rows)),rows[:5]]:
            add([action('parse',value=order),action('save')],version=version)
        valid=[dict(row,lifepoints='7',prayerPoints='2.5')for row in rows]
        for multiplier in ['1','9.99','10','20','NaN','Infinity']:
            for perma in [False,True]:
                config=dict(version=version)
                if perma:config['permadeath']=None
                add([action('profile',value=dict(skills=valid,totalEXP='123.5',exp_multiplier=multiplier,milestone=dict(combatMilestone='5',skillMilestone='6'))),action('save')],**config)
        for field,bad in [('id','bad'),('id','24'),('dynamic','bad'),('static',None),('experience','bad'),('lifepoints',None),('prayerPoints',None)]:
            data=[dict(row)for row in valid];data[3][field]=bad;add([action('parse',value=data),action('save')],version=version)
    for divisor in ['0','-1','2','Infinity','NaN']:add([action('correct',value=divisor),action('save')])
    for entity in ['player','npc','other']:
        add([action('xp',skill=0,value='1000'),action('copy',value=dict(experience=[str(i+0.5)for i in range(24)],staticLevels=list(range(24)),dynamicLevels=list(range(24,48)),prayerPoints='5.25',lifepoints=31,lifepointsIncrease=8,experienceGained='500',experienceMultiplier='17',combatMilestone=5,skillMilestone=6,lastTrainedSkill=20,lastXpGain=1000,lifepointsUpdate=True)),action('save')],entity=entity,tutorial=True)
    for id,dynamic in [('24','bad'),('-1','bad'),('24','7')]:add([action('parse',value=[dict(id=id,dynamic=dynamic,static='4',experience='5')])])
    for name in [None,'attack','ATTACK','Hitpoints','Hıtpoınts','Hiſtory','attacK','Summoning','not a skill']:cases.append(dict(kind='name',name=name))
    for level in list(range(-5,513))+[10000]:cases.append(dict(kind='curve',level=level))
    for exp in ['NaN','Infinity','-Infinity','-1','0','82','82.5','83','13034430','13034430.5','13034431']:
        cases.append(dict(kind='xp_level',value=exp))
    rng=random.Random(530)
    for _ in range(100):
        add([action('rawLevels',static=[rng.randrange(-100,200)for _ in range(24)],dynamic=[rng.randrange(-100,200)for _ in range(24)]),action('combat'),action('restore'),action('save')],entity=rng.choice(['player','npc','other']),perk=bool(rng.randrange(2)))
    return cases
def generate(check,benchmark_file=None):
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
    stdlib=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));classpath=os.pathsep.join(map(str,[classes,stdlib,json_jar]));java=[]
    for name,text in ADAPTERS.items():path=RUN/'adapters'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf-8');java.append(path)
    for name in ['core/game/node/entity/skill/Skills.java','content/global/handlers/item/equipment/brawling_gloves/BrawlingGloves.java']:
        path=SERVER/name;read(path);java.append(path)
    # Compile the original skill-index mapping; the surrounding assist/gear and
    # plugin implementations remain explicit controllable boundaries.
    assist=read(SERVER/'core/game/node/entity/player/link/request/assist/AssistSession.java')
    path=RUN/'adapters/core/game/node/entity/player/link/request/assist/AssistSession.java';path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text('package core.game.node.entity.player.link.request.assist;import core.game.node.entity.skill.Skills;public class AssistSession {public core.game.node.entity.player.Player current,other;public boolean translateExperience(core.game.node.entity.player.Player p,int slot,double xp,double mod){oracle.Model.call("assist_translate",slot,oracle.Model.bits(xp),oracle.Model.bits(mod));return oracle.Model.flag("translate");}public core.game.node.entity.player.Player getPlayer(){return oracle.Model.flag("assistOwn")?current:other;}public boolean isRestricted(){return oracle.Model.flag("restricted");}public boolean[] getSkills(){boolean[] flags=new boolean[9];java.util.Arrays.fill(flags,oracle.Model.flag("assistEnabled"));return flags;}'+compiler.method(assist,'public final int getSkillIndex(')+'}',encoding='utf-8');java.append(path)
    read(HERE/'Model.java');java.append(HERE/'Model.java')
    run(['javac','--release','11','-encoding','UTF-8','-cp',classpath,'-d',classes,*java])
    parser=read(SERVER/'core/game/node/entity/player/info/login/PlayerSaveParser.kt');saver=read(SERVER/'core/game/node/entity/player/info/login/PlayerSaver.kt')
    methods=RUN/'SkillMethods.kt';methods.write_text('import core.game.node.entity.player.Player\nimport core.game.node.entity.skill.Skills\nimport org.json.simple.*\nclass SkillParser(val player:Player){var saveFile:JSONObject?=null;'+compiler.method(parser,'fun parseSkills(')+'}\nclass SkillSaver(val player:Player){'+compiler.method(saver,'fun saveSkills(')+'}',encoding='utf-8')
    run(['java','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',classpath,'-d',classes,methods])
    read(HERE/'SkillOracle.java');read(HERE/'SkillBenchmark.java');run(['javac','--release','11','-encoding','UTF-8','-cp',classpath,'-d',classes,HERE/'SkillOracle.java',HERE/'SkillBenchmark.java'])
    command=['java','-Dfile.encoding=UTF-8','-cp',classpath,'SkillOracle']
    if compiler.JAVA_HOME:command[0]=str(compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java'))
    def evaluate(inputs):
        input_file=RUN/'input.jsonl';input_file.write_bytes(''.join(json.dumps(case,sort_keys=True,ensure_ascii=True)+'\n'for case in inputs).encode())
        with input_file.open('rb')as stdin:result=subprocess.run(command,cwd=ROOT,stdin=stdin,capture_output=True,timeout=120)
        if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
        output=[json.loads(line)for line in result.stdout.splitlines()if line]
        if len(output)!=len(inputs):raise RuntimeError('Missing skill cases')
        return output
    output=evaluate(requests())
    thresholds=[]
    for case in output:
        if case['kind']=='curve' and 2<=case['level']<=100:
            for offset in [-1,-0.5,0]:thresholds.append(dict(kind='xp_level',value=str(case['expected']+offset)))
    output+=evaluate(thresholds)
    data=''.join(json.dumps(case,sort_keys=True,ensure_ascii=True,separators=(',',':'))+'\n'for case in output).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Unchanged complete Skills and BrawlingGloves enum; verbatim profile parseSkills/saveSkills and assist skill-index mapping. Explicit controlled entity/world ticks, equipment, assist decisions, glove manager, perks, plugins, level-up, packet/event, news and logging adapters. Traces and synchronous callback/failure effects tested; dependent systems and packet bytes are not emulated or claimed complete. No playable-world/full-profile claim.')
    folder=ROOT/'fixtures/skills'
    if check:
        if (folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text())!=provenance:raise RuntimeError('Skills oracle/provenance differs; review before regeneration')
        print(f'{len(output)} original skill cases reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(output)} original skill cases.')
    if benchmark_file:
        benchmark=[*command[:-1],'SkillBenchmark',str(benchmark_file)]
        result=subprocess.run(benchmark,cwd=ROOT,capture_output=True,timeout=120)
        if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
        print(result.stdout.decode().strip())
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);p.add_argument('--benchmark-file',type=pathlib.Path);args=p.parse_args();compiler.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(args.check,args.benchmark_file)
