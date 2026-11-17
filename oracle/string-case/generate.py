"""Pin JDK 11 ROOT string casing, contextual sigma and word-boundary data."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,re,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/string-case-oracle'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');compiler=importlib.util.module_from_spec(spec);spec.loader.exec_module(compiler)
read,run=compiler.read,compiler.run
def generate(check):
    version=run(['java','-version']).stderr.decode(errors='replace')
    if not re.search(r'version "11[.\-"]',version):raise RuntimeError('The string-case reference requires JDK 11; select --java-home.')
    RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'));jar=RUN/'json-simple-1.1.1.jar'
    if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    read(HERE/'CaseOracle.java');cp=os.pathsep.join(map(str,[classes,jar]));run(['javac','--release','11','-cp',cp,'-d',classes,HERE/'CaseOracle.java'])
    command=['java','-Xmx512m','--add-opens','java.base/sun.text=ALL-UNNAMED','--add-opens','java.base/java.lang=ALL-UNNAMED','-Dfile.encoding=UTF-8','-cp',cp,'oracle.CaseOracle']
    model=json.loads(run([*command,'data']).stdout)
    representatives={}
    for lo,hi,cat in model['wordRanges']:
        for c in [lo,hi]:
            if 0xd800<=c<=0xdfff:continue
            representatives[c]=cat
    strings=['','ASCII Timer','İIıi','ΟΣ','ΟΣΑ','AΣ\u0345','AΣ𐐀','\uffffAΣ','AΣ\uffffA','AΣ\x00A','Straße','𐐀𐐁Σ','\u1c89Σ','A\u200dΣ','A.ΣA']
    for cp,_ in model['lowerPairs']:strings.extend([chr(cp),'A'+chr(cp)+'Σ'])
    for cp in representatives:
        for before,after in [('AΣ','A'),('A','Σ'),('AΣ',''),('A','ΣA'),('Σ','A')]:strings.append(before+chr(cp)+after)
    rng=random.Random(530);alphabet=[chr(cp)for cp in representatives]+['A','Σ',' ','İ','σ','ς','\u0345']
    for _ in range(1000):strings.append(''.join(rng.choices(alphabet,k=rng.randrange(1,35))))
    requests=[dict(text=s)for s in dict.fromkeys(strings)];file=RUN/'input.jsonl';file.write_bytes(''.join(json.dumps(c,ensure_ascii=True)+'\n'for c in requests).encode())
    executable=compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if compiler.JAVA_HOME else'java';command[0]=executable
    with file.open('rb')as f:p=subprocess.run(list(map(str,command)),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
    if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
    cases=[json.loads(line)for line in p.stdout.splitlines()if line]
    data=''.join(json.dumps(c,sort_keys=True,ensure_ascii=True,separators=(',',':'))+'\n'for c in cases).encode();table=(json.dumps(model,sort_keys=True,ensure_ascii=True,separators=(',',':'))+'\n').encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(cases),fixtureSha256=hashlib.sha256(data).hexdigest(),dataSha256=hashlib.sha256(table).hexdigest(),scope='JDK 11 String.toLowerCase(Locale.ROOT), actual ROOT BreakIterator boundaries, every code point singleton lower mapping and ConditionalSpecialCasing cased flag, complete ROOT word automaton/category observations via test-only reflection. Native strings exclude unpaired UTF-16. Other locales and general BreakIterator APIs are outside scope.')
    folder=ROOT/'fixtures/string-case';items={'oracle.jsonl':data,'data.json':table,'provenance.json':(json.dumps(provenance,sort_keys=True,indent=2)+'\n').encode()}
    if check:
        for name,b in items.items():
            if(folder/name).read_bytes()!=b:raise RuntimeError('String case '+name+' differs; review before regeneration')
        print(f'{len(cases)} original JDK string-case/word-boundary cases reproduced; all code-point data match.')
    else:
        folder.mkdir(parents=True,exist_ok=True)
        for name,b in items.items():(folder/name).write_bytes(b)
        print(f'Generated {len(cases)} original JDK string-case cases.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();compiler.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
