"""Original chat/dialog writer branches, real Huffman table and character predicates."""
import hashlib,json,pathlib,random,re,struct,subprocess
def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/ui-oracle/chat';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    for name in ['HuffmanCodec','CharUtils']:(run/(name+'.java')).write_text(source(name),encoding='utf-8')
    jag=source('JagString')
    (run/'JagString.java').write_text('''import java.nio.charset.StandardCharsets;
class JagString{byte[] chars;int length;JagString(){}JagString(byte[] data){chars=data;length=data.length;}static JagString parse(String s){return new JagString(s.getBytes(StandardCharsets.ISO_8859_1));}int length(){return length;}byte[] toByteArray(){return java.util.Arrays.copyOf(chars,length);}int encodeString(byte[] dest,int off,int len){System.arraycopy(chars,0,dest,off,len);return len;}static void copy(byte[] from,int a,byte[] to,int b,int n){System.arraycopy(from,a,to,b,n);}
'''+''.join(method(jag,s)for s in ['public final JagString toLowerCase()', 'public final boolean startsWith(', 'public final JagString substring(int from)', 'public final JagString substring(int to, int from)', 'public final boolean isInt()', 'private boolean isValidInt(', 'public final int parseInt()', 'public final int parseHexString(', 'public final long encode37()'])+'}',encoding='utf-8')
    buffer=source('Buffer');(run/'Buffer.java').write_text('class Buffer {byte[] data=new byte[10000];int offset;void p1isaac(int n){p1(n);}'+''.join(method(buffer,s)for s in ['public final void p1(', 'public final void p2(', 'public final void p4(', 'public final void p8(', 'public final void pjstr(', 'public final void psmarts(', 'public final void psize1('])+'}',encoding='utf-8')
    (run/'WordPack.java').write_text('class WordPack{static HuffmanCodec codec;'+method(source('WordPack'),'public static int encode(')+'}',encoding='utf-8')
    runner=source('ScriptRunner');block=method(runner,'if (opcode == 5008)');localized=source('LocalizedText');fields=dict(re.findall(r'(?:public|private) static (?:final )?JagString (\w+) = ([^;]+);',localized))
    def literal(name):
        value=fields[name]
        return value if value.startswith('JagString.parse')else literal(value)
    text='class LocalizedText{'+''.join('static JagString '+name+'='+literal(name)+';'for name in sorted(set(re.findall(r'LocalizedText\.(\w+)',block))))+'}'
    (run/'LocalizedText.java').write_text(text,encoding='utf-8')
    dialogs=''.join(method(runner,'if (opcode == Cs2Opcodes.'+name+')')for name in ['sendIntegerInput','sendNameInput','sendStringInput'])
    code='''import java.io.*;import java.util.*;
class Protocol{static Buffer outboundBuffer;}class LoginManager{static int staffModLevel;static boolean playerUnderage,parentalChatConsent,mapQuickChat;}class client{static int language=0;}class ClientProt{static int CLIENT_CHEAT=44,RESUME_P_COUNTDIALOG=23,RESUME_P_NAMEDIALOG=244,RESUME_P_STRINGDIALOG=65;}
class Cheat{static void execute(JagString text){sendCheatPacket(text);}'''+method(source('Cheat'),'public static void sendCheatPacket(')+'''}
public class Probe{static JagString DOUBLE_COLON=JagString.parse("::");static JagString[] stringStack;static int ssp;static class Cs2Opcodes{static int sendIntegerInput=3104,sendNameInput=3105,sendStringInput=3106;}
static void run(int opcode){JagString string,str1;int c,int3;'''+block.replace('continue;','return;')+dialogs.replace('continue;','return;')+'''}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[] x=line.split(";",-1);if(x[0].equals("H")){WordPack.codec=new HuffmanCodec(HexFormat.of().parseHex(x[1]));continue;}if(x[0].equals("C")){int c=Integer.parseInt(x[1]);System.out.println((CharUtils.isValidChar(c)?1:0)+","+(CharUtils.isLetterOrDigit(c)?1:0)+","+(CharUtils.isLetter(c)?1:0)+","+(CharUtils.isDigit(c)?1:0)+","+CharUtils.toLowerCase(c)+","+CharUtils.toUpperCase(c));continue;}int op=Integer.parseInt(x[0]);LoginManager.staffModLevel=Integer.parseInt(x[2]);LoginManager.playerUnderage=x[3].equals("1");LoginManager.parentalChatConsent=false;LoginManager.mapQuickChat=x[4].equals("1");stringStack=new JagString[]{new JagString(HexFormat.of().parseHex(x[1]))};ssp=1;Protocol.outboundBuffer=new Buffer();run(op);System.out.println(HexFormat.of().formatHex(Arrays.copyOf(Protocol.outboundBuffer.data,Protocol.outboundBuffer.offset)));}}}
'''
    (run/'Probe.java').write_text(code,encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    data=(root/'assets/visual/ui.pack').read_bytes();n=struct.unpack_from('<I',data,8)[0];manifest=json.loads(data[12:12+n]);offset,size=manifest['files']['huffman'];table=data[12+n+offset:12+n+offset+size];inputs=['H;'+table.hex()];cases=[];rng=random.Random(530)
    for _ in range(1024):
        text=rng.choice(['','RED:','Green:','yellow:','flash1:','glow3:','unknown:','::'])+rng.choice(['','wave:','wave2:','SLIDE:','shake:','scroll:'])+''.join(rng.choice('a Bcd345\xe9-!')for _ in range(rng.randrange(1,65)));case=dict(op=5008,text=text,rights=rng.choice([0,2]),restricted=rng.randrange(2),quick=rng.randrange(2));cases.append(case)
    for text in ['', '0','-1','+1','2147483647','-2147483648','2147483648','-2147483649','+','12a','00',' 1','Name Here','Caf\xe9','::hello']:
        for op in [3104,3105,3106]:cases.append(dict(op=op,text=text,rights=0,restricted=0,quick=0))
    inputs.extend(str(c['op'])+';'+c['text'].encode('latin1').hex()+';'+str(c['rights'])+';'+str(c['restricted'])+';'+str(c['quick'])for c in cases)
    characters=list(range(-128,513))+[-2147483648,2147483647,10000,65535];inputs.extend('C;'+str(c)for c in characters)
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==len(cases)+len(characters)
    for c,line in zip(cases,result):c['wire']=line
    chars=[dict(c=c,expected=list(map(int,line.split(','))))for c,line in zip(characters,result[len(cases):])]
    p=root/'fixtures/ui/chat.json';content=json.dumps(dict(sources=hashes,bindings='English language0; real cached Huffman table; cheat command wire only, local command effects unbound; plain opcodes',huffman=table.hex(),cases=cases,characters=chars),separators=(',',':')).encode()+b'\n'
    if check:
        if p.read_bytes()!=content:raise SystemExit('UI chat corpus differs')
    else:p.write_bytes(content)
    print(f'RT4 chat/dialog wires {len(cases)}; character boundaries {len(chars)}')
if __name__=='__main__':
    import importlib.util,sys
    root=pathlib.Path(__file__).resolve().parents[2];spec=importlib.util.spec_from_file_location('visual',root/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);generate(root,v.clean,v.method,'--check'in sys.argv)
