"""Full unchanged GroundSpawnLoader joined to subtype/manager/scheduler."""
import argparse,importlib.util,json,os,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('spawn',ROOT/'oracle/ground-spawn/generate.py');spawn=importlib.util.module_from_spec(spec);spec.loader.exec_module(spawn)
SCOPE='Complete unchanged GroundSpawnLoader.kt including load, GroundItemManager/GroundItem/Pulse and deterministic-clock PulseRunner. Full pinned ground_spawns.json and row/argument/failure/reentrant cases. Native actual subtype/manager are used. Definition/plugin/chunk/player, item constructor dependencies and log sinks are explicit adapters. Log exceptions normalize type categories and omit JVM stack frames; original FileReader runs on isolated UTF8 files. Native UTF8 bounded reader/error read-ahead, full world entry/Take/content remain explicit limits.'


def transform(name,text,source,c):
    text=spawn.transform(name,text,source,c)
    if name=='core/game/system/config/GroundSpawnLoader.kt':
        return source('core/game/system/config/GroundSpawnLoader.kt')[1]
    if name=='core/game/node/item/Item.java':
        text=text.replace('private int idHash,amount;','private String name;private int idHash,amount;')
        text=text.replace('public Item(int id){','public Item(int id,int amount){this(id,amount,1000);}public Item(int id){')
        text=text.replace('definition=oracle.ManagerSupport.definition(getId());','definition=oracle.ManagerSupport.definition(id);name=definition.getName();')
        text=text.replace('public String getName(){return getDefinition().getName();}','public String getName(){return name;}')
    elif name=='core/ServerConstants.kt':
        text=text.replace('var GRAFANA_LOGGING=false;','var GRAFANA_LOGGING=false;val CONFIG_PATH:String? get()=oracle.LoaderSupport.path;')
    elif name=='core/game/world/map/Location.kt':
        _,original=source(name)
        line=next(s.strip()for s in original.splitlines()if'override fun toString()'in s)
        text=text.replace('override fun equals',line+';override fun equals')
    elif name=='core/tools/Log.java':
        text=text.replace('ERR=1,WARN=2','ERR=1,WARN=2,FINE=3')
    elif name=='core/api/API.kt':
        text='package core.api\nfun log(owner:Any,level:Int,message:String){oracle.LoaderSupport.log(level,message)}'
    elif name=='oracle/Support.java':
        text=text.replace('SpawnSupport.reset();','SpawnSupport.reset();LoaderSupport.reset();')
        text=text.replace('SpawnSupport.snapshot(s);return s;','SpawnSupport.snapshot(s);if(Boolean.TRUE.equals(input.get("loader")))LoaderSupport.snapshot(s);return s;')
        text=text.replace('e instanceof Failure?"Failure":e instanceof NullPointerException?"Null":e instanceof IndexOutOfBoundsException?"Index":e.getClass().getSimpleName()','LoaderSupport.category(e.getClass().getName())')
    elif name=='oracle/SpawnSupport.java':
        text=text.replace('default:throw new IllegalArgumentException((String)a.get("op"));','default:return LoaderSupport.act(a);')
    return text


def register(adapter,source,read,c):
    spawn.register(adapter,source,read,c)
    adapter('oracle/LoaderSupport.java',read(HERE/'LoaderSupport.java'))
    read(HERE/'generate.py');read(ROOT/'fixtures/ground-spawn-loader/ground_spawns.json')


def requests():
    cases=[]
    def a(op,**kw):return dict(op=op,**kw)
    def add(actions,**kw):cases.append(dict(manager=True,spawns=True,loader=True,actions=actions,**kw))
    def data(loc='{1,3207,3216,0,3}',id=1):return json.dumps([dict(item_id=id,loc_data=loc)],ensure_ascii=False)
    def load(text):return a('loaderLoad',text=text)
    add([a('loaderLoad',pinned=True),a('loaderRead')])
    for loc in ['', '-', '--', '{}', '{1,3207,3216,0,3}-{}-{2,3208,3216,0,3}', '1,3207,3216,0,3-2,3208,3216,0,3', '-1,3207,3216,0,3', '1,3207,3216,-1,3', '1,3207,3216,0,-1', '1,3207,3216,0,3,ignored', '1,3207,3216,0,', '1,3207,3216,0', '1,3207,3216,0,2147483648', ' 1,3207,3216,0,3', '1, 3207,3216,0,3', '1,3207,3216,0, 3', '+1,+3207,+3216,+0,+3', '١,٣٢٠٧,٣٢١٦,٠,٣', '{1,32{07},3216,0,3}', '0,65535,65536,5,65535', '1,no-x,no-y,no-z,no-rate', 'no-amount,no-x,no-y,no-z,1', '1,no-x,3216,0,1']:
        add([load(data(loc)),a('loaderRead')])
    for id in [None,1,1.0,True,'+1','١',' 1','2147483648',-1,65536,{},[]]:add([load(data(id=id))])
    for text in ['null','{}','[null]','[3]','[{}]','[{"item_id":1}]','[{"item_id":1,"loc_data":null}]','[,{"item_id":1 "loc_data":"1,3207,3216,0,3"},]','[','[{"item_id":1,"loc_data":[]} ]']:
        add([load(text),a('loaderRead')])
    first=dict(item_id=1,loc_data='{1,3207,3216,0,1}-{2,3208,3216,0,2}')
    third=dict(item_id=2,loc_data='{3,3209,3216,0,3}')
    text=json.dumps([first,dict(item_id='invalid',loc_data=''),third])
    for event in ['item','default','ticks','trade','lookup','dirty','loaderError','loaderCount']:
        for at in [1,2,3]:add([load(text),a('managerPulse'),a('loaderRead')],failures={event:at})
    for event in ['item','default','trade','lookup','dirty','loaderError','loaderCount']:
        add([load(text),a('loaderRead')],hooks=[dict(event=event,actions=[load(data(id=3))])])
    add([load(data()),a('loaderLoad',missing=True),a('loaderRead'),load('null'),a('loaderRead'),load('[]')])
    add([a('definition',id=1,name='First name'),load(data()),a('definition',id=1,name='Later name'),a('loaderStrings')])
    for name in [None,'£ Coins','Ångström']:
        add([a('definition',id=1,name=name),load(data()),a('loaderStrings')])
    add([load(data()),a('setGround',g=0,location=None),a('loaderStrings'),a('managerDestroy',g=0),a('spawnWorld')])
    for at in [1,2]:
        add([a('definition',id=1,name='Constructor name'),load(data()),a('loaderStrings')],hooks=[dict(event='item',at=at,actions=[a('definition',id=1,name='Changed name')])])
    add([load(json.dumps([dict(item_id='invalid',loc_data='bad: java.lang.NullPointerException',note='" / \\ £')],ensure_ascii=False))])
    return cases


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);args=p.parse_args()
    c=spawn.manager.base.c;c.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None)
    lock=json.loads((ROOT/'upstream.lock.json').read_text());raw=c.run(['git','-C',ROOT/'reference/server','show',lock['server']['commit']+':Server/data/configs/ground_spawns.json']).stdout
    folder=ROOT/'fixtures/ground-spawn-loader';folder.mkdir(parents=True,exist_ok=True);config=folder/'ground_spawns.json'
    if args.check:assert config.read_bytes()==raw
    else:config.write_bytes(raw)
    spawn.manager.base.generate(args.check,sys.modules[__name__],'ground-spawn-loader')
