"""Run original character activity/quest/global records and verbatim JSON save/load methods."""
import argparse,copy,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/saved-data-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');compiler=importlib.util.module_from_spec(spec);spec.loader.exec_module(compiler)
read=compiler.read;run=compiler.run
def requests(defaults):
    cases=[]
    def add(actions):cases.append(dict(kind='saved',actions=actions))
    add([dict(op='save')])
    for target,key in [('activity','activityData'),('quest','questData'),('global','globalData')]:
        default=defaults[key]
        for field,value in default.items():
            for variant in ['missing',None,{},'bad']:
                row=copy.deepcopy(default)
                if variant=='missing':row.pop(field)
                else:row[field]=variant
                add([dict(op='parse',target=target,value=row),dict(op='save')])
            if isinstance(value,list):
                for length in [0,1,len(value)-1,len(value),len(value)+1]:
                    row=copy.deepcopy(default);row[field]=[copy.deepcopy(value[0]) for _ in range(length)]
                    add([dict(op='parse',target=target,value=row),dict(op='save')])
            elif isinstance(value,str):
                for sample in ['-1','+1','０','2147483647','2147483648','-2147483648','9223372036854775807','-9223372036854775808']:
                    row=copy.deepcopy(default);row[field]=sample;add([dict(op='parse',target=target,value=row),dict(op='save')])
        add([dict(op='parse',target=target,value=default),dict(op='parse',target=target,value={})])
    for field in ['activityData','questData','globalData']:
        for value in [None,{},[],False,'bad']:
            row=copy.deepcopy(defaults);row[field]=value;add([dict(op='sections',value=row),dict(op='save')])
    for missing in ['questData','globalData']:
        row=copy.deepcopy(defaults);row['activityData']['pestPoints']='123';row['questData']['dragonSlayerPlanks']='456';row[missing]=None
        add([dict(op='sections',value=row),dict(op='save')])
    for value in ['NaN','Infinity','-Infinity','-0.0','1e23','0x1.23456789abcdefp100','0x0.0000000000001p-1022']:
        row=copy.deepcopy(defaults['globalData']);row['assistExperience']=[value]*9;add([dict(op='parse',target='global',value=row),dict(op='save')])
    rng=random.Random(530)
    for _ in range(24):
        row=copy.deepcopy(defaults)
        for section in row.values():
            for field,value in section.items():
                if isinstance(value,bool):section[field]=bool(rng.randrange(2))
                elif isinstance(value,str):section[field]=str(rng.randrange(-120,120))
                elif isinstance(value,list):
                    for index,item in enumerate(value):
                        if isinstance(item,bool):value[index]=bool(rng.randrange(2))
                        elif isinstance(item,str):value[index]=str(rng.randrange(-120,120))
        add([dict(op='sections',value=row),dict(op='save'),dict(op='sections',value=defaults),dict(op='save')])
    def call(target,method,args=(),types=(),**extra):return dict(op='call',target=target,method=method,args=args,types=types,**extra)
    for start in [-2**31,-1,0,499,500,10000,2**31-1]:
        for amount in [-2**31,-1,0,1,2**31-1]:
            add([call('activity','setPestPoints',[start],['int']),call('activity','increasePestPoints',[amount],['int']),call('activity','decreasePestPoints',[amount],['int']),call('activity','setBarrowKills',[start],['int']),call('activity','setKolodionStage',[amount],['int']),call('activity','setLastBorkBattle',[amount],['long'])])
    for method in ['incrementPizazz','decrementPizazz']:
        for index in [-1,0,3,4]:
            add([call('activity',method,[index],['int']),call('activity',method,[index,2**31-1],['int','int']),call('activity',method,[index,1],['int','int'])])
    for method in ['updateWarriorTokens','updateBountyHunterRate','updateBountyRogueRate']:
        add([call('activity',method,[2**31-1],['int']),call('activity',method,[1],['int']),call('activity',method,[-1],['int'])])
    for literal in [False,True]:
        for name in ['lobster','wizard','silk','bowl','ship','memorized','repaired','ned','poured','milk','egg','flour','gave','other']:
            add([call('quest','setDragonSlayerAttribute',[name,True],['String','boolean'],literal=literal),call('quest','getDragonSlayerAttribute',[name],['String'],literal=literal),call('quest','getDragonSlayerItem',[name],['String'],literal=literal),call('quest','setCooksAssistant',[name,True],['String','boolean'],literal=literal),call('quest','getCookAssist',[name],['String'],literal=literal)])
    for index in [-2**31,-1,0,6,7,8,2**31-1]:add([call('quest','getDesertTreasureItem',[index],['int']),dict(op='item',index=index,id=-1,amount=-20),call('quest','getDesertTreasureItem',[index],['int']),dict(op='item',index=index,id=None),dict(op='save')])
    for index in [-2**31,-1,0,1,3,4,45]:
        actions=[]
        for method in ['setTravelLog','hasTravelLog','removeTravelLog','hasStrongholdReward','setAbyssCharge','hasAbyssCharge','getRcDecay']:actions.append(call('global',method,[index],['int']))
        actions.append(call('global','resetAbyss'));add(actions)
    for now in [-2**63,-1,0,1000,2**63-1]:
        for until in [-2**63,-1,0,999,1000,1001,2**63-1]:add([call('global','setDoubleExp',[until],['long']),dict(op='clock',value=now),call('global','hasDoubleExp')])
    for ordinal in range(3):add([call('global','setGodBook',[ordinal],['GodBook']),call('global','hasCompletedGodBook',[ordinal],['GodBook'])])
    add([call('global','setSavedLocation',[-2**31,2**31-1,-1],['int','int','int']),call('global','hasReadPlaques')])
    bits=[0,1,2,0x8000000000000000,0x7ff0000000000000,0xfff0000000000000,0x7ff8000000000000]
    for exponent in range(2048):
        for mantissa in [0,1,(1<<52)-1]:bits.append((exponent<<52)|mantissa)
    bits += [rng.getrandbits(64) for _ in range(4000)]
    import struct
    for exponent in range(-323,309):
        value=float('1e'+str(exponent));raw=struct.unpack('>Q',struct.pack('>d',value))[0]
        bits += [raw-1,raw,raw+1,raw|(1<<63)]
    for value in bits:cases.append(dict(kind='double_format',bits=f'{value:016x}'))
    for text in ['','1','-0.0','+NaN','-NaN','NaN','Infinity','-Infinity','Infinityf','--1','+-1','1f','1d',' .5 ','1e-400','1e400','1e+1','1e-1','1.0e','１２','0x1.8p1','-0x0.1p-1070','0x1.fffffffffffffp1023','0x1.fffffffffffff8p1023','0x1p-1075','0x1.0000000000001p-1075','0x1p+','0x1p-','0x0p999999999999999999999999','0x1p-999999999999999999999999','0x1p999999999999999999999999']:
        cases.append(dict(kind='double_parse',text=text))
    for _ in range(1000):
        mantissa=rng.getrandbits(rng.randrange(1,160));exponent=rng.randrange(-1200,1200)
        cases.append(dict(kind='double_parse',text=('-'if rng.randrange(2)else'')+'0x'+f'{mantissa:x}'+'p'+str(exponent)))
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
    def write(name,text):
        path=RUN/'adapters'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf-8');return path
    java=[]
    for name in ['ActivityData','QuestData','GlobalData','SavedData']:
        path=SERVER/'core/game/node/entity/player/link'/(name+'.java');read(path);java.append(path)
    java.append(write('Player.java','package core.game.node.entity.player;public class Player {private final core.game.node.entity.player.link.SavedData savedData=new core.game.node.entity.player.link.SavedData(this);public core.game.node.entity.player.link.SavedData getSavedData(){return savedData;}}'))
    java.append(write('System.java','package core.game.node.entity.player.link;public class System {public static long now=0;public static long currentTimeMillis(){return now;}}'))
    god=read(SERVER/'content/data/GodBook.java');names=['HOLY_BOOK','BOOK_OF_BALANCE','UNHOLY_BOOK'];assert all(name+'('in god for name in names)
    java.append(write('GodBook.java','package content.data;public enum GodBook {'+','.join(names)+'}'))
    # Retain original ID packing and amount construction. Definition/Node behavior
    # is outside the saved-field oracle and deliberately not presented as item parity.
    item=read(SERVER/'core/game/node/item/Item.java')
    java.append(write('Item.java','package core.game.node.item;public class Item {private int idHash;private int amount;public Item(int id,int amount){this.idHash=id<<16|1000;this.amount=amount;}'+compiler.method(item,'public int getId()')+compiler.method(item,'public int getAmount()')+compiler.method(item,'public int getIdHash()')+'}'))
    run(['javac','--release','11','-encoding','UTF-8','-cp',classpath,'-d',classes,*java])
    saver=read(SERVER/'core/game/node/entity/player/info/login/PlayerSaver.kt');parser=read(SERVER/'core/game/node/entity/player/info/login/PlayerSaveParser.kt')
    source='import core.game.node.entity.player.Player\nimport org.json.simple.JSONObject\nimport org.json.simple.JSONArray\nclass SavedSaver(val player:Player){'+''.join(compiler.method(saver,'fun '+method+'(')for method in ['saveSavedData','saveActivityData','saveQuestData','saveGlobalData'])+'}\nclass SavedParser(val player:Player){var saveFile:JSONObject?=null;'+compiler.method(parser,'fun parseSavedData(')+'}'
    kotlin=write('SavedMethods.kt',source)
    run(['java','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',classpath,'-d',classes,kotlin])
    read(HERE/'SavedOracle.java');run(['javac','--release','11','-encoding','UTF-8','-cp',classpath,'-d',classes,HERE/'SavedOracle.java'])
    def evaluate(cases):
        command=['java','-Dfile.encoding=UTF-8','-cp',classpath,'SavedOracle']
        if compiler.JAVA_HOME:command[0]=str(compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java'))
        result=subprocess.run(command,cwd=ROOT,input=''.join(json.dumps(case,ensure_ascii=True)+'\n'for case in cases).encode(),capture_output=True)
        if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
        output=[json.loads(line)for line in result.stdout.splitlines()if line]
        if len(output)!=len(cases):raise RuntimeError('Missing oracle output')
        return output
    defaults=evaluate([dict(kind='saved',actions=[dict(op='save')])])[0]['expected'][0]['root']
    output=evaluate(requests(defaults));data=''.join(json.dumps(case,ensure_ascii=True,sort_keys=True,separators=(',',':'))+'\n'for case in output).encode()
    read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged ActivityData/QuestData/GlobalData/SavedData; verbatim PlayerSaveParser.parseSavedData and PlayerSaver saveSavedData/saveActivityData/saveQuestData/saveGlobalData methods. Explicit player/clock/GodBook-ordinal/item-field adapters and reflection state setup. Includes JDK 11 Double formatting/parsing. No item definitions/interaction, complete profile or playable world claim.')
    folder=ROOT/'fixtures/saved-data'
    if check:
        if (folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text())!=provenance:raise RuntimeError('Saved data oracle/provenance differs; review before regeneration')
        print(f'{len(output)} original saved-data/numeric cases reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(output)} original saved-data/numeric cases.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);args=p.parse_args();compiler.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(args.check)
