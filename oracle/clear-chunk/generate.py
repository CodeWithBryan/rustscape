"""Complete unchanged ClearRegionChunk.send over the joined native packet state."""
import argparse,importlib.util,os,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('manager',ROOT/'oracle/ground-manager/generate.py');manager=importlib.util.module_from_spec(spec);spec.loader.exec_module(manager)
SCOPE='Complete unchanged ClearRegionChunk and ClearChunkContext with original Location scene-coordinate getters and native chunk_sync.clear. Joined original Item/GroundItem/packet manager dependencies. Player/session/region callbacks are explicit recorded services; ISAAC disabled. Covers104tile bounds, failed prefixes and captured last-scene/live-base reentry, not full world/profile/activity or capacity.'


def transform(name,text,source,c):
    text=manager.transform(name,text,source,c)
    if name=='core/game/world/map/RegionChunk.java':
        text=text[:-1]+c.method(source(name)[1],'public Location getCurrentBase()')+'}'
    if name=='oracle/Support.java':text=text.replace('default:return ManagerSupport.act(a);','case"clearChunk":new core.net.packet.out.ClearRegionChunk().send(new core.net.packet.context.ClearChunkContext(player(a),chunk(a)));return null;default:return ManagerSupport.act(a);')
    return text


def register(adapter,source,read,c):
    manager.register(adapter,source,read,c)
    for name in ['core/net/packet/out/ClearRegionChunk.java','core/net/packet/context/ClearChunkContext.java','core/net/packet/OutgoingPacket.java']:
        adapter(name,source(name)[1])
    adapter('core/game/world/update/MapChunkRenderer.java','package core.game.world.update;public class MapChunkRenderer{public static final int BUILD_AREA_SIZE=13;}')
    read(HERE/'generate.py')


def requests():
    cases=[]
    def a(op,**kw):return dict(op=op,**kw)
    def add(actions,**kw):cases.append(dict(manager=True,actions=actions,**kw))
    for x in [-9,-1,0,1,7,8,95,96,103,104,105]:
        for y in [-9,-1,0,1,7,8,95,96,103,104,105]:add([a('base',location=[3152+x,3168+y,0]),a('clearChunk')])
    for scene in [None,[-16384,-16384,3],[32767,32767,1],[0,0,0],[3207,3216,2]]:
        add([a('player',scene=scene),a('clearChunk')])
    for event in ['scene','send']:
        add([a('clearChunk'),a('clearChunk')],failures={event:1})
        for change in [a('base',location=[3255,3271,0]),a('base',location=[3256,3272,0]),a('player',scene=None),a('player',scene=[3264,3264,0])]:
            add([a('clearChunk'),a('clearChunk')],hooks=[dict(event=event,actions=[change])])
    return cases


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);args=p.parse_args()
    manager.base.c.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None)
    manager.base.generate(args.check,sys.modules[__name__],'clear-chunk')
