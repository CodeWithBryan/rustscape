"""Reproduce all pinned minimap differential corpora without changing fixtures."""
import argparse, pathlib, subprocess, sys
ROOT=pathlib.Path(__file__).resolve().parents[2]
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--pack',type=pathlib.Path,help='Prepared pack whose raw metadata is checked')
a=p.parse_args()
for name in ['generate','scene','metadata','state','wire','edge']:
    command=[sys.executable,str(ROOT/f'oracle/minimap/{name}.py'),'--check']
    if name=='metadata' and a.pack:command+=['--pack',str(a.pack.resolve())]
    subprocess.run(command,cwd=ROOT,check=True)
