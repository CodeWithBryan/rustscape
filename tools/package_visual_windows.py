"""Package the native Windows workbench and its pinned offline inputs."""
import argparse,json,os,pathlib,shutil,subprocess

ROOT=pathlib.Path(__file__).resolve().parents[1]

def package(output):
    if os.name!='nt':raise SystemExit('Build this package on Windows.')
    subprocess.run(['cargo','build','--release','-p','rs09-client'],cwd=ROOT,check=True)
    metadata=json.loads(subprocess.check_output(['cargo','metadata','--format-version','1','--no-deps'],cwd=ROOT))
    files={'rs09-client.exe':pathlib.Path(metadata['target_directory'])/'release/rs09-client.exe'}
    for name in ['demo.pack','ui-players.pack','animations.pack']:files['assets/visual/'+name]=ROOT/'assets/visual'/name
    for gender in ['male','female']:files[f'assets/visual/player-{gender}.bin']=ROOT/f'runs/visual/player-{gender}.bin'
    for source in (ROOT/'fixtures/players/forms').iterdir():
        if source.is_file():files['assets/visual/forms/'+source.name]=source
    files['assets/visual/live/lumbridge.pack']=ROOT/'assets/visual/live/lumbridge.pack'
    files['assets/visual/npc-render-replay.json']=ROOT/'fixtures/players/npc-render-replay.json'
    files['assets/visual/npc-overheads-replay.json']=ROOT/'fixtures/players/npc-overheads-replay.json'
    files['assets/visual/npc-nearby-replay.json']=ROOT/'fixtures/players/npc-nearby-replay.json'
    files['assets/visual/slot-render-replay.json']=ROOT/'fixtures/players/slot-render-replay.json'
    for name in ['ui-gameframe.bin','ui-preview.bin','ui-shop.bin','ui-dialogue.bin','ui-player-dialogue.bin','ui-item-message.bin']:files['runs/visual/'+name]=ROOT/'runs/visual'/name
    for source in files.values():
        if not source.is_file():raise SystemExit(f'Missing {source}; prepare the visual/UI packs and ui_preview fixtures first.')
    for name,source in files.items():
        target=output/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target)
    (output/'Launch client.cmd').write_text('@echo off\ncd /d "%~dp0"\nrs09-client.exe --ui game --replay runs/visual/ui-gameframe.bin --replay-now 1\n',encoding='ascii')
    for gender in ['male','female']:
        (output/f'Launch {gender} equipment.cmd').write_text(f'@echo off\ncd /d "%~dp0"\nrs09-client.exe --appearance-preview assets/visual/player-{gender}.bin --ui game --replay runs/visual/ui-gameframe.bin --replay-now 1\n',encoding='ascii')
    for label,replay in [('NPC dialogue','ui-dialogue'),('Player dialogue','ui-player-dialogue'),('Item message','ui-item-message')]:
        (output/f'Launch {label}.cmd').write_text(f'@echo off\ncd /d "%~dp0"\nrs09-client.exe --pack assets/visual/live/lumbridge.pack --ui game --replay runs/visual/{replay}.bin --replay-now 1\n',encoding='ascii')
    (output/'Launch live client.cmd').write_text('''@echo off
cd /d "%~dp0"
powershell -NoProfile -Command "$world=Read-Host 'World IP:port (Enter for 127.0.0.1:43596)'; if (!$world) {$world='127.0.0.1:43596'}; $name=Read-Host 'Username'; $secret=Read-Host 'Password' -AsSecureString; $env:RS09_CLIENT_PASSWORD=[System.Net.NetworkCredential]::new('', $secret).Password; try { & '.\\rs09-client.exe' --connect $world --username $name --pack 'assets/visual/live/lumbridge.pack' } finally { Remove-Item Env:\\RS09_CLIENT_PASSWORD -ErrorAction SilentlyContinue }"
''',encoding='ascii')
    (output/'Launch animation and audio.cmd').write_text((output/'Launch live client.cmd').read_text(encoding='ascii').replace('43596','43598'),encoding='ascii')
    (output/'Launch movement probe.cmd').write_text((output/'Launch live client.cmd').read_text(encoding='ascii').replace('43596','43600'),encoding='ascii')
    (output/'Launch NPC update preview.cmd').write_text('@echo off\ncd /d "%~dp0"\nrs09-client.exe --pack assets/visual/live/lumbridge.pack --npc-replay assets/visual/npc-render-replay.json\n',encoding='ascii')
    (output/'Launch NPC overhead preview.cmd').write_text('@echo off\ncd /d "%~dp0"\nrs09-client.exe --pack assets/visual/live/lumbridge.pack --npc-replay assets/visual/npc-overheads-replay.json\n',encoding='ascii')
    (output/'Launch nearby NPC preview.cmd').write_text('@echo off\ncd /d "%~dp0"\nrs09-client.exe --pack assets/visual/live/lumbridge.pack --npc-replay assets/visual/npc-nearby-replay.json\n',encoding='ascii')
    (output/'Launch per-part animation preview.cmd').write_text('@echo off\ncd /d "%~dp0"\nrs09-client.exe --pack assets/visual/live/lumbridge.pack --npc-replay assets/visual/slot-render-replay.json --appearance-preview assets/visual/player-male.bin --ui game --replay runs/visual/ui-gameframe.bin --replay-now 1\n',encoding='ascii')
    (output/'README.txt').write_text('2009scape Rust visual compatibility workbench\n\nDouble-click Launch client.cmd for offline UI previews, or Launch live client.cmd for a native TCP development world. No Java or Python is needed to run this package.\nThe live launcher asks for an endpoint, username and hidden password; credentials are not saved. The server must already be running.\n\nOffline: F5 inventory/game frame; F6 populated bank; F7 shop; F8 equipment. F9 interfaces/workbench controls; F10 diagnostics.\nClick ground to request walking; right-click for original cached actions; middle-drag to orbit.\n\nOffline fixtures use synthetic inventory and stock. Live mode disables those previews and synthetic NPCs.\nLive login, normal scene rebuild and player membership, kit/equipment body rendering and original model widgets are implemented. The current server fixture has stationary players only. Authoritative movement, doors, bank/shop/content outcomes, authoritative NPC spawning, NPC player transforms, unrestricted streaming/planes and full client parity remain pending.\nThe pinned sources and licenses are documented in the repository THIRD_PARTY_NOTICES.md.\n',encoding='utf-8')
    with (output/'README.txt').open('a',encoding='utf-8') as readme:
        readme.write('\nLaunch animation and audio.cmd uses the separate explicit probe on 127.0.0.1:43598. It sends original wave863 and sound127 requests every six seconds. Use a unique development username and the development password testing. The probe server must be running. Primary action clocks/blending and finite synthesized server sounds are implemented; music, continuous ambient emitters and full actor/movement effects remain separate work.\n')
    with (output/'README.txt').open('a',encoding='utf-8') as readme:
        readme.write('\nLaunch NPC update preview.cmd runs a separate offline packet replay with three original NPC indices, walking, spawn/removal/reappearance, wave/dance animation and four sound requests. Launch NPC overhead preview.cmd includes original chat, health bars and hit splats. Launch nearby NPC preview.cmd displays the expanded nearby type set and hidden/visible morph phases in synthetic positions. These require no server and do not run NPC AI or combat. The browser offers the same previews. Live NPC32 decoding/rendering uses the same Rust path, with 28 prepared nearby/preview types including one morph child. Slot/spot graphics, ambient emitters and general body streaming remain pending.\n')
    with (output/'README.txt').open('a',encoding='utf-8') as readme:
        readme.write('\nLaunch movement probe.cmd selects the separate controlled walking/running world on TCP 127.0.0.1:43600. The browser movement preset selects WS 43601. A running probe server is required. Full profile initialization, NPC AI, doors/agility/bank/shop/content and general world behavior remain incomplete. Existing ordinary and animation/audio launchers keep their original defaults.\n')
    with (output/'README.txt').open('a',encoding='utf-8') as readme:
        readme.write('\nMale/female equipment launchers use explicit offline rune appearances and perform no server equipment transaction. Body assembly includes gender offsets, original recolours, per-slot BAS geometry transforms and primary sequence equipment overrides. Runtime movement remains the default-acceleration BAS subset; NPC player transforms, slot-masked overlays and full gameplay remain unfinished.\n')
    for name in ['LICENSE','THIRD_PARTY_NOTICES.md','upstream.lock.json']:shutil.copyfile(ROOT/name,output/name)
    print(f'Native Windows package: {output}',flush=True)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=pathlib.Path,default=ROOT/'runs/windows-client');args=parser.parse_args();package(args.output.resolve())
