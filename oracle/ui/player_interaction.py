"""Original player menus, SET_INTERACTION, name/title state and action writes.

Actor state, menu insertion and the byte-backed JagString storage are explicit
adapters. This does not execute server combat, trade or follow handlers.
"""
import hashlib, importlib.util, json, pathlib, random, re, subprocess, sys


def generate(root, clean, method, check=False):
    run = root / 'runs/ui-oracle/player-interaction'
    run.mkdir(parents=True, exist_ok=True)
    hashes = {}

    def source(name):
        raw = (root / f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n', b'\n')
        hashes[name] = hashlib.sha256(raw).hexdigest()
        return clean(raw.decode())

    mini = source('MiniMenu')
    constants = ''.join(re.findall(r'public static final JagString \w+ = [^;]+;', mini))
    codes = {n: int(v) for n, v in re.findall(r'public static final int (\w+) = (\d+);', mini)}
    writes = ''
    for action in [30, 6, 31, 29, 10, 44, 37, 57, 1, 15]:
        name = next(n for n, value in codes.items() if value == action)
        block = method(mini, 'if (actionCode == ' + name + ')')
        writes += f'if(actionCode=={action}){{' + ''.join(line for line in block.splitlines() if 'Protocol.outboundBuffer.' in line) + '}'
    (run / 'MiniMenu.java').write_text('''class MiniMenu {
static int size,itemTargetMode,targetMask,targetCursorId;static boolean isTargeting;
static JagString selectedObjText,targetOpBase,targetVerb;static int selectedObjId,selectedObjSlot;
static final short[] PLAYER_OPTION_ACTION_CODES={30,6,31,29,10,44,37,57};
static short[] actions=new short[500];static JagString[] ops=new JagString[500],opBases=new JagString[500];
static int[] aa=new int[500],bb=new int[500],cc=new int[500];static long[] kk=new long[500];
static void add(int cursor,long key,JagString target,int a,short action,JagString option,int b){actions[size]=action;ops[size]=option;opBases[size]=target;aa[size]=a;bb[size]=b;cc[size]=cursor;kk[size]=key;size++;}
''' + constants + method(mini, 'public static void addPlayerEntries(') + method(mini, 'public static JagString getCombatLevelColor(') + '''
static byte[] wire(int actionCode,int keyInt){Protocol.outboundBuffer=new Buffer(new byte[1000]);int targetInterfaceId=0x12345678,targetChildId=-1;''' + writes + '''return java.util.Arrays.copyOf(Protocol.outboundBuffer.data,Protocol.outboundBuffer.offset);}}
class MiniMap{static int useCursor,selectedComponentId;}class client{static int game;}
''', encoding='utf-8')
    (run / 'Player.java').write_text('''class Player {static JagString[] options=new JagString[8];static int[] cursors=new int[8];static boolean[] secondaryOptions=new boolean[8];int combatLevel,combatLevelWithSummoning,combatRange,skill,team,titleIndex;JagString username;''' + method(source('Player'), 'public final JagString getName()') + '}class PlayerList{static Player self;static JagString[] playerNames,playerNames2;static final int[] nameSlotCounts={1,4};}', encoding='utf-8')
    source('PlayerList')
    buffer = source('Buffer')
    (run / 'Buffer.java').write_text('class Buffer{byte[] data;int offset;Buffer(byte[] d){data=d;}void p1isaac(int n){p1(n);}' + ''.join(method(buffer, s) for s in ['public final int g1()', 'public final int ig2add()', 'public final JagString gjstr()', 'public final void p1(', 'public final void p2(', 'public final void p2add(', 'public final void ip2(', 'public final void ip2add(', 'public final void mp4(', 'public final void ip4(']) + '}', encoding='utf-8')
    protocol = source('Protocol')
    (run / 'Protocol.java').write_text('class ServerProt{static final int SET_INTERACTION=44;}class Protocol{static Buffer inboundBuffer,outboundBuffer;static int opcode;static boolean update(){' + method(protocol, 'if (opcode == ServerProt.SET_INTERACTION)') + 'return false;}}', encoding='utf-8')
    jag = source('JagString')
    (run / 'JagString.java').write_text('''class JagString {byte[] chars;int length;static final JagString NULL_STRING=new JagString("null");JagString(){}JagString(String s){chars=s.getBytes(java.nio.charset.StandardCharsets.ISO_8859_1);length=chars.length;}String text(){return new String(chars,0,length,java.nio.charset.StandardCharsets.ISO_8859_1);}static JagString decodeString(byte[] d,int n,int o){return new JagString(new String(d,o,n,java.nio.charset.StandardCharsets.ISO_8859_1));}int length(){return length;}boolean equalsIgnoreCase(JagString s){return text().equalsIgnoreCase(s.text());}static void copy(byte[] a,int p,byte[] b,int q,int n){System.arraycopy(a,p,b,q,n);}static JagString parse(String s){String out="";for(int i=0;i<s.length();i++){int c=s.charAt(i);if(c>=40&&c<=45&&i+1<s.length())c=(c-40)*43+s.charAt(++i)-48;if(c!=0)out+=(char)c;}return new JagString(out);}static JagString parseInt(int n){return new JagString(""+n);}
''' + ''.join(method(jag, s) for s in ['public static JagString concatenate(', 'public static JagString concatenateRange(', 'public final JagString toTitleCase()']) + '}', encoding='utf-8')
    base = source('Base37')
    (run / 'Base37.java').write_text('class Base37{static final byte[] DECODE_CHARS="_abcdefghijklmnopqrstuvwxyz0123456789".getBytes();' + method(base, 'public static JagString decode37(') + '}', encoding='utf-8')
    titles = method(source('ScriptRunner'), 'if (opcode == 5422)')
    (run / 'Probe.java').write_text('''import java.io.*;import java.util.*;
class LocalizedText{static JagString LEVEL=new JagString("level: "),RATING=new JagString("rating: "),SKILL=new JagString("skill: "),ATTACK=new JagString("Attack"),USE=new JagString("Use");}
public class Probe{
static String hex(JagString s){return s==null?"-":HexFormat.of().formatHex(Arrays.copyOf(s.chars,s.length));}
static String labels(JagString[] a){return a==null?"~":String.join(",",Arrays.stream(a).map(Probe::hex).toArray(String[]::new));}
static String options(){String s=labels(Player.options);for(int i=0;i<8;i++)s+=":"+Player.cursors[i]+":"+(Player.secondaryOptions[i]?1:0);return s;}
static Player actor(int[] a,int o){Player p=new Player();p.username=new JagString("Sir Test 7");p.combatLevel=a[o];p.combatLevelWithSummoning=a[o+1];p.combatRange=a[o+2];p.skill=a[o+3];p.team=a[o+4];return p;}
static JagString unhex(String h){return h.equals("~")?null:new JagString(new String(HexFormat.of().parseHex(h),java.nio.charset.StandardCharsets.ISO_8859_1));}
static void titles(int index,JagString prefix,JagString suffix){int isp=1,ssp=2;int[]intStack={index};JagString[]stringStack={prefix,suffix};int opcode=5422;int int2;JagString str1,string;for(int once=0;once<1;once++){''' + titles + '''}}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]x=line.split(";",-1);
if(x[0].equals("W")){int[]a=Arrays.stream(x[1].split(",")).mapToInt(Integer::parseInt).toArray();MiniMenu.selectedObjId=1277;MiniMenu.selectedObjSlot=27;MiniMap.selectedComponentId=149<<16;System.out.println(HexFormat.of().formatHex(MiniMenu.wire(a[0],a[1])));continue;}
if(x[0].equals("U")){Player.options=new JagString[8];Player.cursors=new int[8];Player.secondaryOptions=new boolean[8];String out="";for(String h:x[1].split(",")){Protocol.inboundBuffer=new Buffer(HexFormat.of().parseHex(h));Protocol.opcode=44;Protocol.update();out+=(out.isEmpty()?"":";")+options();}System.out.println(out);continue;}
if(x[0].equals("N")){client.game=Integer.parseInt(x[1]);PlayerList.playerNames=null;PlayerList.playerNames2=null;String out="";for(String e:x[2].split("/")){String[]v=e.split(",",-1);String error="ok";if(v[0].equals("T")){try{titles(Integer.parseInt(v[1]),unhex(v[2]),unhex(v[3]));}catch(RuntimeException ex){error="error";}}else{Player p=new Player();p.titleIndex=Integer.parseInt(v[2]);try{p.username=Base37.decode37(Long.parseLong(v[1])).toTitleCase();error=hex(p.getName());}catch(RuntimeException ex){error="error";}}out+=(out.isEmpty()?"":";")+error+":"+labels(PlayerList.playerNames)+":"+labels(PlayerList.playerNames2);}System.out.println(out);continue;}
int[]a=Arrays.stream(x[1].split(",")).mapToInt(Integer::parseInt).toArray();client.game=a[0];PlayerList.self=actor(a,1);Player other=a[17]!=0?PlayerList.self:actor(a,6);PlayerList.playerNames=null;PlayerList.playerNames2=null;
MiniMenu.size=0;for(int i=0;i<a[16];i++)MiniMenu.add(-1,0,new JagString(""),0,(short)(i==0?60:1005),new JagString(i==0?"Walk here":"seed"),0);
for(int i=0;i<8;i++){Player.options[i]=(a[11]>>i&1)==0?null:new JagString(i==0?"aTtAcK":"Option"+i);Player.cursors[i]=i*13-1;Player.secondaryOptions[i]=(a[12]>>i&1)!=0;}
MiniMenu.itemTargetMode=a[13];MiniMenu.isTargeting=a[14]!=0;MiniMenu.targetMask=a[15];MiniMenu.targetCursorId=37;MiniMap.useCursor=29;MiniMenu.selectedObjText=new JagString("Seed");MiniMenu.targetOpBase=new JagString("Spell");MiniMenu.targetVerb=new JagString("Cast");
MiniMenu.addPlayerEntries(321,43,other,27);String out="";for(int i=0;i<MiniMenu.size;i++){if(MiniMenu.ops[i].text().equals("seed"))continue;out+=(out.isEmpty()?"":",")+MiniMenu.actions[i]+":"+MiniMenu.kk[i]+":"+MiniMenu.aa[i]+":"+MiniMenu.bb[i]+":"+MiniMenu.cc[i]+":"+hex(MiniMenu.ops[i])+":"+hex(MiniMenu.opBases[i]);}System.out.println(out);
}}}
''', encoding='utf-8')
    subprocess.run(['javac', '-encoding', 'UTF-8', '-d', str(run), *map(str, run.glob('*.java'))], check=True)
    rng = random.Random(53044)
    menus, wires, updates, names = [], [], [], []
    levels = [0, 3, 10, 80, 126, 138, 65535, -2147483648, 2147483647]
    for i in range(1024):
        args = [rng.randrange(2)] + [rng.choice(levels), rng.choice(levels), rng.choice([-1, 0, 5, 20]), rng.choice([0, 0, 0, 99]), rng.randrange(4)] * 2
        for j in range(6, 11):
            args[j] = rng.choice(levels) if j < 8 else rng.choice([-1, 0, 5, 20]) if j == 8 else rng.choice([0, 0, 0, 99]) if j == 9 else rng.randrange(4)
        args += [rng.randrange(256), rng.randrange(256), rng.randrange(2), rng.randrange(2), rng.choice([0, 1, 8, 9, 127]), rng.choice([1, 2, 399, 400]), int(i % 23 == 0)]
        menus.append(dict(args=args))
    for action in [30, 6, 31, 29, 10, 44, 37, 57, 1, 15]:
        for index in [-1, 0, 1, 127, 128, 255, 256, 2047, 32767, 32768, 65535, 65536]:
            wires.append(dict(action=action, index=index))
    for _ in range(128):
        frames = []
        for _ in range(8):
            cursor, top, slot = rng.choice([0, 1, 127, 128, 65534, 65535]), rng.choice([0, 1, 2, 255]), rng.choice([0, 1, 2, 7, 8, 9, 255])
            label = rng.choice(['Attack', 'Follow', 'Trade with', 'null', 'NuLl', '', '\u00c9change'])
            frames.append(bytes([(cursor+128)&255, cursor>>8, top, slot]).hex() + label.encode('latin-1').hex() + '00')
        updates.append(dict(frames=frames))
    for game in [0, 1]:
        for i in range(64):
            value = rng.choice([-1, 0, 1, 37, 6582952005840035280, 6582952005840035281, 137151, 958096752418, 312924363863392])
            events = [f'T,0,{"Lord ".encode().hex()},', f'N,{value},{rng.randrange(4)}', f'T,{rng.choice([-1,0,1,3,4])},{rng.choice(["", "Sir "]).encode().hex()},{rng.choice(["", " the Brave"]).encode().hex()}', f'N,{value},0', 'T,0,,', f'N,{value},0']
            events += ['T,0,53697220,~', f'N,{value},0', 'T,0,~,']
            names.append(dict(game=game, events=events))
    inputs = ['M;' + ','.join(map(str, c['args'])) for c in menus] + [f'W;{c["action"]},{c["index"]}' for c in wires] + ['U;' + ','.join(c['frames']) for c in updates] + [f'N;{c["game"]};' + '/'.join(c['events']) for c in names]
    result = subprocess.run(['java', '-cp', str(run), 'Probe'], input='\n'.join(inputs)+'\n', capture_output=True, text=True, check=True).stdout.splitlines()
    assert len(result) == len(inputs)
    for c, output in zip(menus + wires + updates + names, result):
        c['expected'] = output
    content = json.dumps(dict(sources=hashes, bindings=__doc__, menus=menus, wires=wires, updates=updates, names=names), separators=(',', ':')).encode() + b'\n'
    path = root / 'fixtures/ui/player-interaction.json'
    if check:
        if path.read_bytes() != content:
            raise SystemExit('Player interaction corpus differs from original')
    else:
        path.write_bytes(content)
    print(f'RT4 player interactions: {len(menus)} menus, {len(wires)} wires, {len(updates)} update sequences, {len(names)} name/title sequences')


if __name__ == '__main__':
    root = pathlib.Path(__file__).resolve().parents[2]
    spec = importlib.util.spec_from_file_location('visual', root/'oracle/visual/generate.py')
    v = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(v)
    generate(root, v.clean, v.method, '--check' in sys.argv)
