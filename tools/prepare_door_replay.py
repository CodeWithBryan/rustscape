"""Embed verified native-server door packets as raw workbench input, including WASM."""
import argparse,hashlib,json,pathlib,struct

ROOT=pathlib.Path(__file__).resolve().parents[1]
def prepare(pack,trace,metadata,output,repeat):
    wire=trace.read_bytes();meta=json.loads(metadata.read_text(encoding='utf-8'))
    digest=lambda b:hashlib.sha256(b).hexdigest()
    if meta['revision']!=530 or meta['isaac'] or meta['direction']!='ServerToClient' or meta['wireSha256']!=digest(wire):raise SystemExit('Trace provenance or SHA differs')
    if not 1<=repeat<=100:raise SystemExit('Repeat count must be 1..100')
    data=pack.read_bytes()
    if data[:8]!=b'RS09VIS1':raise SystemExit('Invalid visual pack')
    n=struct.unpack_from('<I',data,8)[0];manifest=json.loads(data[12:12+n]);blob=bytearray(data[12+n:]);payload=wire*repeat
    if 'replay.s2c'in manifest['files']:raise SystemExit('Input pack already has a replay')
    manifest['files']['replay.s2c']=[len(blob),len(payload)];blob.extend(payload)
    header=json.dumps(manifest,separators=(',',':')).encode();result=b'RS09VIS1'+struct.pack('<I',len(header))+header+blob
    output.parent.mkdir(parents=True,exist_ok=True);output.write_bytes(result)
    provenance=dict(source_pack_sha256=digest(data),pack_sha256=digest(result),original_wire_sha256=digest(wire),embedded_wire_sha256=digest(payload),repeat=repeat,server_metadata=meta,scope='Unchanged server packet bytes; repeat is presentation playback, not server timing or live world entry')
    output.with_suffix('.provenance.json').write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8');print(f'Trace replay pack: {output}')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--pack',type=pathlib.Path,default=ROOT/'assets/visual/tour/lumbridge-door.pack');p.add_argument('--trace',type=pathlib.Path,default=ROOT/'runs/scene-oracle/lumbridge-door.s2c');p.add_argument('--metadata',type=pathlib.Path,default=ROOT/'runs/scene-oracle/lumbridge-door.json');p.add_argument('--output',type=pathlib.Path,default=ROOT/'assets/visual/tour/lumbridge-door-replay.pack');p.add_argument('--repeat',type=int,default=1);a=p.parse_args();prepare(a.pack,a.trace,a.metadata,a.output,a.repeat)
