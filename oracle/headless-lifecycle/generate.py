"""Unchanged RT4 Protocol liveness blocks, isolated from rendering/UI callbacks."""
import argparse, hashlib, importlib.util, json, pathlib, random, subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
RUN = ROOT / 'runs/headless-lifecycle-oracle'
FIX = ROOT / 'fixtures/headless-lifecycle/oracle.json'

def generate(check, java_home):
    lock = json.loads((ROOT / 'upstream.lock.json').read_text(encoding='utf-8'))
    reference = ROOT / 'reference/client'
    assert subprocess.check_output(['git', '-C', str(reference), 'rev-parse', 'HEAD'], text=True).strip() == lock['client']['commit']
    subprocess.run(['git', '-C', str(reference), 'diff', '--quiet', 'HEAD', '--'], check=True)
    source_path = reference / 'client/src/main/java/rt4/Protocol.java'
    raw = source_path.read_bytes().replace(b'\r\n', b'\n')
    source = raw.decode('utf-8')
    spec = importlib.util.spec_from_file_location('visual', ROOT / 'oracle/visual/generate.py')
    visual = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(visual)
    source = visual.clean(source)
    read = 'LoginManager.ticksSinceLastPacket++;\n' + visual.method(source, 'if (LoginManager.ticksSinceLastPacket > 750)')
    keepalive = visual.method(source, 'if (ticksSinceWrite > 50)')
    tail = source[source.index('if (ticksSinceWrite > 50)'):]
    write = visual.method(tail, 'try {') + visual.method(tail, 'catch (IOException ioEx)')
    received = 'LoginManager.ticksSinceLastPacket = 0;'
    assert source.count(received) == 1 and source.count('ticksSinceWrite++;') == 1
    code = r'''
import java.io.*;import java.util.*;
class LoginManager {static int ticksSinceLastPacket;}
class Packet {byte[] data=new byte[65536];int offset;void p1isaac(int value){data[offset++]=(byte)value;}}
class Socket {boolean fail;void write(byte[] data,int count)throws IOException{if(fail)throw new IOException("controlled write failure");Probe.sent.write(data,0,count);}}
public class Probe {
static int ticksSinceWrite;static Packet outboundBuffer;static Socket socket;static boolean closed;static ByteArrayOutputStream sent;
static void handleConnectionLost(){closed=true;}
static void tick(){READ ticksSinceWrite++; KEEP WRITE}
static String snapshot(){return "["+LoginManager.ticksSinceLastPacket+","+ticksSinceWrite+","+closed+",\""+HexFormat.of().formatHex(sent.toByteArray())+"\",\""+HexFormat.of().formatHex(Arrays.copyOf(outboundBuffer.data,outboundBuffer.offset))+"\"]";}
public static void main(String[]args)throws Exception{BufferedReader input=new BufferedReader(new InputStreamReader(System.in));String line;
while((line=input.readLine())!=null){String[] all=line.split(";");String[] seed=all[0].split(",");LoginManager.ticksSinceLastPacket=Integer.parseInt(seed[0]);ticksSinceWrite=Integer.parseInt(seed[1]);outboundBuffer=new Packet();socket=new Socket();closed=false;sent=new ByteArrayOutputStream();List<String> states=new ArrayList<>();
for(int i=1;i<all.length;i++){String[] op=all[i].split(",");switch(op[0]){case "T":for(int n=0;n<Integer.parseInt(op[1])&&!closed;n++)tick();break;case "R":RECEIVED break;case "P":for(byte b:HexFormat.of().parseHex(op[1]))outboundBuffer.data[outboundBuffer.offset++]=b;break;case "S":socket=op[1].equals("null")?null:new Socket();if(socket!=null)socket.fail=op[1].equals("fail");break;}states.add(snapshot());}System.out.println("["+String.join(",",states)+"]");}}
}'''.replace('READ', read).replace('KEEP', keepalive).replace('WRITE', write).replace('RECEIVED', received)
    RUN.mkdir(parents=True, exist_ok=True)
    (RUN / 'Probe.java').write_text(code, encoding='utf-8')
    def exe(name): return str(java_home / 'bin' / (name + '.exe')) if java_home else name
    subprocess.run([exe('javac'), '-encoding', 'UTF-8', '-d', str(RUN), str(RUN / 'Probe.java')], check=True)
    cases = []
    def add(read, write, ops): cases.append(dict(read=read, write=write, ops=ops))
    add(0, 0, [['T', 50], ['T', 1], ['T', 50], ['T', 1], ['R'], ['T', 750], ['T', 1]])
    add(749, 50, [['P', '1234'], ['T', 1], ['T', 1]])
    add(0, 49, [['S', 'fail'], ['T', 2]])
    add(0, 49, [['S', 'null'], ['T', 8], ['S', 'ok'], ['T', 1]])
    for initial_read in [-2147483648, -1, 0, 749, 750, 2147483647]:
        for initial_write in [-2147483648, -1, 0, 49, 50, 2147483647]:
            add(initial_read, initial_write, [['T', 1], ['R'], ['P', 'aabb'], ['T', 2]])
    rng = random.Random(53093)
    for _ in range(96):
        ops = []
        for _ in range(24):
            n = rng.randrange(5)
            ops.append(['T', rng.randrange(1, 90)] if n < 3 else ['R'] if n == 3 else ['P', bytes([rng.randrange(256)]).hex()])
        add(rng.randrange(0, 700), rng.randrange(0, 60), ops)
    encoded = [';'.join([f"{c['read']},{c['write']}"] + [','.join(map(str, op)) for op in c['ops']]) for c in cases]
    result = subprocess.run([exe('java'), '-cp', str(RUN), 'Probe'], input='\n'.join(encoded)+'\n', text=True, encoding='utf-8', capture_output=True, check=True)
    rows = result.stdout.splitlines()
    assert len(rows) == len(cases)
    for case, row in zip(cases, rows): case['expected'] = json.loads(row)
    fixture = dict(format=1, upstream=lock['client'], sourceSha256=hashlib.sha256(raw).hexdigest(), blocks={k:hashlib.sha256(v.encode()).hexdigest() for k,v in dict(read=read,keepalive=keepalive,write=write,received=received).items()}, scope='Unchanged Protocol read counter/timeout, write counter/keepalive and socket-write try/catch blocks. ISAAC disabled as pinned. In-memory socket records writes or fails; connection-lost marks this attempt closed. Rendering/UI/input/gameplay callbacks, reconnect and transport scheduling are outside this corpus.', cases=cases)
    content = (json.dumps(fixture, separators=(',', ':'))+'\n').encode('utf-8')
    if check: assert FIX.read_bytes() == content, 'headless lifecycle corpus differs'
    else: FIX.parent.mkdir(parents=True, exist_ok=True); FIX.write_bytes(content)
    print(f'{len(cases)} original RT4 liveness scenarios '+('reproduced.' if check else 'generated.'))

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    generate(args.check, args.java_home)
