"""Pinned drawOverheads/addHit/chat loop/font waves; explicit projection/assets."""
import hashlib,json,random,subprocess

SUPPORT=r'''
import java.io.*;import java.util.*;
class JagString {String value;JagString(String s){value=s;}static JagString parseInt(int n){return new JagString(""+n);}int length(){return value.length();}}
class client {static int loop;static Object js5Archive8;}
class PluginRepository {static void NPCOverheadDraw(Npc n,int x,int y){}static void PlayerOverheadDraw(Player p,int x,int y){}}
class PathingEntity {int px,py,height;boolean visible;JagString chatMessage;int chatLoops=100,chatColor,chatEffect,hitpointsBar,hitpointsBarVisibleUntil=-1000;int[]hitVisibleUntil=new int[4],hitDamages=new int[4],hitTypes=new int[4];boolean isVisible(){return visible;}int getModelHeight(){return height;} ADD_HIT }
class Player extends PathingEntity {int skullHeadIcon=-1,prayerHeadIcon=-1;JagString username;}
class Npc extends PathingEntity {NpcType type;}
class NpcType {int[]multiNpcs;NpcType selected;int headicon=-1,iconHeight=-1,hitBarId=-1;NpcType getMultiNpc(){return selected;}}
class PlayerList {static Player self;static int size;static Player[]players=new Player[2048];static int[]ids=new int[2048];}
class NpcList {static int size;static Npc[]npcs=new Npc[32768];static int[]ids=new int[32768];}
class FriendsList {static Set<String>names=new HashSet<>();static boolean contains(JagString n){return names.contains(n.value);}}
class Chat {static int publicFilter;}
class VarpDomain {static int chatEffectsDisabled;}
class MapMarker {int type,actorTargetId,arrowSpriteId;}
class MiniMap {static MapMarker[]hintMapMarkers=new MapMarker[4];}
class GlRenderer {static boolean enabled=true;}
class GlRaster {static void setClipRegion(int a,int b,int c,int d){Probe.clip(a,b,c,d);}static void setClip(int a,int b,int c,int d){Probe.clip(a,b,c,d);}}
class SoftwareRaster {static void shrinkClip(int a,int b,int c,int d){Probe.clip(a,b,c,d);}static void setClip(int a,int b,int c,int d){Probe.clip(a,b,c,d);}}
class Sprite {int group,frame,width,height;Sprite(int g,int f,int w,int h){group=g;frame=f;width=w;height=h;}void render(int x,int y){Probe.draws.add("{\"op\":\"sprite\",\"group\":"+group+",\"frame\":"+frame+",\"at\":["+x+","+y+"]}");}}
class Sprites {static Sprite[]hitbars=make(-2,2,30,5),hitmarks=make(-1,8,24,24),headiconPks=make(-4,8,24,24),headiconPrayers=make(-3,8,24,24),headhints=make(-5,8,24,24);static Sprite[]make(int g,int n,int w,int h){Sprite[]s=new Sprite[n];for(int i=0;i<n;i++)s[i]=new Sprite(g,i,w,h);return s;}}
class HitBarCache extends HashMap<Integer,Sprite[]> {void put(Sprite[]v,int key){super.put(key,v);}}
class HitBarList {static HitBarCache hitBars=new HitBarCache();}
class SpriteLoader {static Sprite[]loadAlphaSprites(int n,Object a){return null;}}
class FontService {boolean small;int lineHeight=12;FontService(boolean s){small=s;}int getStringWidth(JagString t){return t.value.length()*6;}void text(int mode,JagString t,int x,int y,int color,int tick,int age){Probe.draws.add("{\"op\":\"text\",\"small\":"+small+",\"mode\":"+mode+",\"text\":"+Probe.q(t.value)+",\"at\":["+x+","+y+"],\"color\":"+color+",\"tick\":"+tick+",\"age\":"+age+"}");}void renderCenter(JagString t,int x,int y,int c,int s){text(0,t,x,y,c,0,0);}void renderLeft(JagString t,int x,int y,int c,int s){text(4,t,x,y,c,0,0);}void renderWave(JagString t,int x,int y,int c,int tick){text(1,t,x,y,c,tick,0);}void renderWave2(JagString t,int x,int y,int c,int tick){text(2,t,x,y,c,tick,0);}void renderShake(JagString t,int x,int y,int c,int tick,int age){text(3,t,x,y,c,tick,age);}}
class Fonts {static FontService b12Full=new FontService(false),p11Full=new FontService(true);}
class ScriptRunner {static int screenX,screenY,renderCycle;static void setOverheadScreenCoordinateOffsets(int hh,int zh,PathingEntity e,int zv,int height,int hw){screenX=e.px;screenY=e.py-height/4;} DRAW_OVERHEADS }
class FontOffsets {void setColors(int c,int s){}int getStringWidth(JagString t){return t.length()*6;}void renderOffset(JagString t,int x,int y,int[]xs,int[]ys){List<String>out=new ArrayList<>();for(int i=0;i<t.length();i++)out.add("["+(xs==null?0:xs[i])+","+(ys==null?0:ys[i])+"]");System.out.println("["+String.join(",",out)+"]");} FONT_METHODS }
public class Probe {
static List<String>draws=new ArrayList<>();static void clip(int a,int b,int c,int d){draws.add("{\"op\":\"clip\",\"rect\":["+a+","+b+","+c+","+d+"]}");}static String q(String s){return "\""+s.replace("\\","\\\\").replace("\"","\\\"")+"\"";}
static void state(){Player p=PlayerList.self;List<String>hits=new ArrayList<>();for(int i=0;i<4;i++)hits.add("["+p.hitTypes[i]+","+p.hitDamages[i]+","+p.hitVisibleUntil[i]+"]");System.out.println("["+"["+String.join(",",hits)+"],"+(p.chatMessage==null?"null":"["+q(p.chatMessage.value)+","+p.chatLoops+"]")+"]");}
public static void main(String[]args)throws Exception {BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]parts=line.split(";",-1);String op=parts[0];
if(op.equals("W")){int effect=Integer.parseInt(parts[1]),len=Integer.parseInt(parts[2]),tick=Integer.parseInt(parts[3]),age=Integer.parseInt(parts[4]);FontOffsets f=new FontOffsets();JagString text=new JagString("A".repeat(len));if(effect==1)f.renderWave(text,0,0,0,tick);else if(effect==2)f.renderWave2(text,0,0,0,tick);else f.renderShake(text,0,0,0,tick,age);continue;}
if(op.equals("R")){PlayerList.self=new Player();PlayerList.players[2047]=PlayerList.self;PlayerList.size=NpcList.size=0;state();continue;}
if(op.equals("H")){PlayerList.self.addHit(Integer.parseInt(parts[1]),Integer.parseInt(parts[2]),Integer.parseInt(parts[3]));state();continue;}
if(op.equals("C")){PlayerList.self.chatMessage=new JagString(parts[2]);PlayerList.self.chatLoops=Integer.parseInt(parts[1]);state();continue;}
if(op.equals("T")){OverheadChat.loop();state();continue;}
draws.clear();FriendsList.names.clear();PlayerList.size=NpcList.size=0;Arrays.fill(MiniMap.hintMapMarkers,null);HitBarList.hitBars.clear();HitBarList.hitBars.put(77,Sprites.make(77,2,48,7));HitBarList.hitBars.put(78,Sprites.make(78,2,32,5));HitBarList.hitBars.put(79,Sprites.make(79,3,24,9));
int[]c=Arrays.stream(parts[1].split(",")).mapToInt(Integer::parseInt).toArray();client.loop=c[0];ScriptRunner.renderCycle=c[1];Chat.publicFilter=c[6];VarpDomain.chatEffectsDisabled=c[7];GlRenderer.enabled=c[8]!=0;
for(int k=2;k<parts.length;k++){String[]a=parts[k].split(",",-1);if(a[0].equals("I")){MapMarker m=new MapMarker();m.type=Integer.parseInt(a[1])==1?1:10;m.actorTargetId=Integer.parseInt(a[2]);m.arrowSpriteId=Integer.parseInt(a[3]);MiniMap.hintMapMarkers[Integer.parseInt(a[4])]=m;continue;}
int[]v=new int[29];for(int j=0;j<v.length;j++)v[j]=Integer.parseInt(a[j]);PathingEntity e;
if(v[0]!=0){Npc n=new Npc();n.type=new NpcType();n.type.headicon=v[4];n.type.iconHeight=v[5];n.type.hitBarId=v[6];if(a.length>30&&a[30].equals("1")){n.type.multiNpcs=new int[]{1};n.type.selected=new NpcType();n.type.selected.headicon=Integer.parseInt(a[31]);n.type.selected.iconHeight=Integer.parseInt(a[32]);n.type.selected.hitBarId=Integer.parseInt(a[33]);}NpcList.npcs[v[1]]=n;NpcList.ids[NpcList.size++]=v[1];e=n;}else{Player p=new Player();p.skullHeadIcon=v[7];p.prayerHeadIcon=v[8];p.username=new JagString("actor"+v[1]);if(v[9]!=0)FriendsList.names.add(p.username.value);if(k==2){PlayerList.self=p;PlayerList.players[2047]=p;}else{PlayerList.players[v[1]]=p;PlayerList.ids[PlayerList.size++]=v[1];}e=p;}
e.visible=v[2]!=0;e.height=v[3];e.hitpointsBar=v[10];e.hitpointsBarVisibleUntil=v[11];e.chatLoops=v[12];e.chatColor=v[13];e.chatEffect=v[14];e.px=v[15];e.py=v[16];for(int j=0;j<4;j++){e.hitTypes[j]=v[17+j*3];e.hitDamages[j]=v[18+j*3];e.hitVisibleUntil[j]=v[19+j*3];}if(!a[29].equals("-"))e.chatMessage=new JagString(new String(HexFormat.of().parseHex(a[29]),java.nio.charset.StandardCharsets.ISO_8859_1));
}
ScriptRunner.drawOverheads(c[3],c[4],c[2],256,c[5],256);System.out.println("["+String.join(",",draws)+"]");
}}}
'''

def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/players-oracle/overheads';run.mkdir(parents=True,exist_ok=True);sources={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');sources[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    draw=method(source('ScriptRunner'),'public static void drawOverheads(')
    hit=method(source('PathingEntity'),'public final void addHit(')
    chat=source('OverheadChat');font=source('Font')
    waves=''.join(method(font,'public final void '+s+'(')for s in ['renderWave','renderWave2','renderShake'])
    (run/'OverheadChat.java').write_text(chat,encoding='utf-8')
    (run/'Probe.java').write_text(SUPPORT.replace('ADD_HIT',hit).replace('DRAW_OVERHEADS',draw).replace('FONT_METHODS',waves),encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),str(run/'Probe.java'),str(run/'OverheadChat.java')],check=True)
    rng=random.Random(53070);lines=[];cases=[]
    for scenario in range(512):
        cycle=rng.choice([0,10,19,20,100,1000,2147483600]);render=rng.randrange(500)
        config=dict(cycle=cycle,render_cycle=render,viewport=[rng.randrange(30),rng.randrange(30),765,503],public_filter=rng.randrange(4),effects_disabled=int(rng.randrange(5)==0),font_height=12,prayer_count=8,default_bar=[30,5],custom_bars={'77':[48,7],'78':[32,5]},hints=[])
        tokens=['D',','.join(map(str,[cycle,render,*config['viewport'],config['public_filter'],int(config['effects_disabled']),scenario%2]))];actors=[];positions=[]
        count=rng.choice([1,2,4,8,12,60]);players=rng.randrange(1,count+1)
        for i in range(count):
            npc=i>=players;index=2047 if i==0 else i
            style=dict(headicon=rng.choice([-1,0,3,7,8]),icon_height=rng.choice([-1,0,128,240]),hitbar=rng.choice([-1,77,78,79]))if npc else None
            icons=[rng.choice([-1,0,3]),rng.choice([-1,1,7])]if not npc else[-1,-1]
            loops=rng.choice([0,1,24,25,49,50,74,75,100,125,126,149,150]);text=rng.choice([None,'','Hello','Hello there','RuneScape','a longer chat line'])
            signed=lambda n:(n+2**31)%2**32-2**31
            state=dict(hits=[dict(kind=rng.randrange(4),damage=rng.randrange(32768),until=rng.choice([0,cycle,signed(cycle+1),signed(cycle+70)]))for _ in range(4)],bar=rng.choice([0,1,64,128,254,255]),bar_until=rng.choice([-1000,cycle,signed(cycle+300)]),chat=None if text is None else dict(text=text,color=rng.randrange(13),effect=rng.randrange(7),loops=loops))
            head=dict(headicon=(style['headicon']+1)%8,icon_height=64 if style['icon_height']==-1 else 240,hitbar=-1)if npc and scenario%4==0 else None
            actor=dict(index=index,visible=rng.randrange(5)!=0,npc=style,npc_head=head,model_height=rng.choice([64,128,200,240]),icons=icons,friend=bool(rng.randrange(2)),state=state);actors.append(actor)
            p=[rng.choice([-1,200,210,220,400]),rng.choice([150,160,200,250])];positions.append(p)
            raw=[int(npc),index,int(actor['visible']),actor['model_height'],style['headicon']if npc else -1,style['icon_height']if npc else -1,style['hitbar']if npc else -1,*icons,int(actor['friend']),state['bar'],state['bar_until'],loops,state['chat']['color']if text is not None else 0,state['chat']['effect']if text is not None else 0,*p]
            for h in state['hits']:raw.extend([h['kind'],h['damage'],h['until']])
            extra=',0'if head is None else',1,'+','.join(map(str,[head['headicon'],head['icon_height'],head['hitbar']]))
            tokens.append(','.join(map(str,raw))+','+('-'if text is None else text.encode().hex())+extra)
        for i in range(rng.randrange(5)):
            a=rng.choice(actors);hint=dict(npc=a['npc']is not None,target=a['index'],sprite=rng.randrange(4));config['hints'].append(hint);tokens.append(f"I,{int(hint['npc'])},{hint['target']},{hint['sprite']},{i}")
        cases.append(dict(config=config,actors=actors,positions=positions));lines.append(';'.join(tokens))
    states=[]
    for scenario in range(32):
        commands=[['R'],['C',rng.choice([1,2,100,150]),'hello']]
        for i in range(96):commands.append(['T']if i%3==0 else['H',rng.randrange(4),rng.choice([0,1,69,70,71,100,2147483600,-2147483600]),rng.randrange(32768)])
        for command in commands:states.append(dict(command=command));lines.append(';'.join(map(str,command)))
    offsets=[]
    for i in range(512):
        row=dict(effect=rng.randrange(1,4),length=rng.randrange(100),tick=rng.choice([0,1,5,20,100,1000,2147483647,-2147483648]),age=rng.choice([0,1,24,25,49,50,56,100,125,150]));offsets.append(row);lines.append(';'.join(map(str,['W',*row.values()])))
    output=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(lines)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(output)==len(lines)
    for row,line in zip([*cases,*states,*offsets],output):row['expected']=json.loads(line)
    content=json.dumps(dict(sources=sources,bindings='Unchanged ScriptRunner.drawOverheads, PathingEntity.addHit, OverheadChat.loop, Font.renderWave/renderWave2/renderShake. Explicit projected coordinates, font width/height, sprite dimensions/cache availability, friend filter and no-op plugin services. No projection/font pixel or content parity claim.',cases=cases,states=states,offsets=offsets),separators=(',',':')).encode()+b'\n';path=root/'fixtures/players/overheads.json'
    if check:
        if path.read_bytes()!=content:raise SystemExit('Overheads corpus differs')
    else:path.write_bytes(content)
    print(f'Original overhead drawing: {len(cases)} scenes, {len(states)} hit/chat commands, {len(offsets)} font effects')
