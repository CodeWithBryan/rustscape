"""Reproduce original PCM, ambient, metadata, synthesis and varp-prefix corpora."""
import pathlib,subprocess,sys
ROOT=pathlib.Path(__file__).resolve().parents[2]
for name in ['pcm','ambient','metadata','synthesis','varps','npc']:
    subprocess.run([sys.executable,str(ROOT/f'oracle/audio/{name}.py'),'--check'],cwd=ROOT,check=True)
