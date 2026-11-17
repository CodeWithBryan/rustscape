"""Owned, loopback-only MariaDB instance for original/native storage comparisons."""
import hashlib
import json
import os
import pathlib
import socket
import shutil
import subprocess
import time
import urllib.request
import uuid
import zipfile

ROOT=pathlib.Path(__file__).resolve().parents[2]
TOOLS=ROOT/'runs/sql-tools'
FLAGS=subprocess.CREATE_NO_WINDOW if os.name=='nt' else 0

def assets():
    if os.name!='nt':raise RuntimeError('The pinned portable database harness currently targets Windows')
    objects=json.loads((ROOT/'oracle/sql/artifacts.lock.json').read_text(encoding='utf-8'))
    TOOLS.mkdir(parents=True,exist_ok=True)
    for name,entry in objects.items():
        target=TOOLS/name
        if not target.is_file() or target.stat().st_size!=entry['size'] or hashlib.sha256(target.read_bytes()).hexdigest()!=entry['sha256']:
            with urllib.request.urlopen(entry['url'],timeout=60) as response:data=response.read()
            if len(data)!=entry['size'] or hashlib.sha256(data).hexdigest()!=entry['sha256']:raise RuntimeError(f'SQL tool artifact differs: {name}')
            target.write_bytes(data)
    binary=TOOLS/'mariadb-10.4.14-winx64/bin/mysqld.exe'
    if not binary.is_file():
        with zipfile.ZipFile(TOOLS/'mariadb.zip') as archive:archive.extractall(TOOLS)
    return binary.parent

class Database:
    def __init__(self,tls=False):
        self.bin=assets();self.root=ROOT/'runs/sql-db'/uuid.uuid4().hex
        self.root.mkdir(parents=True,exist_ok=False);self.process=None;self.log=None;self.owned=False;self.tls=tls
        with socket.socket() as bound:bound.bind(('127.0.0.1',0));self.port=bound.getsockname()[1]
    def __enter__(self):
        initialized=subprocess.run([str(self.bin/'mysql_install_db.exe'),'--datadir='+str(self.root/'data'),'--port='+str(self.port),'--silent'],
            capture_output=True,creationflags=FLAGS,timeout=120)
        (self.root/'initialize.log').write_bytes(initialized.stdout+initialized.stderr)
        if initialized.returncode:raise RuntimeError('Isolated SQL initialization failed; inspect '+str(self.root/'initialize.log'))
        if self.tls:
            openssl=shutil.which('openssl')
            if not openssl:raise RuntimeError('OpenSSL command is required to create the temporary SQL TLS certificate')
            result=subprocess.run([openssl,'req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(self.root/'data/server-key.pem'),'-out',str(self.root/'data/server-cert.pem'),
                '-subj','/CN=localhost','-addext','subjectAltName=DNS:localhost,IP:127.0.0.1','-days','2'],capture_output=True,creationflags=FLAGS,timeout=60)
            (self.root/'tls.log').write_bytes(result.stdout+result.stderr)
            if result.returncode:raise RuntimeError('Temporary SQL TLS certificate generation failed')
        self.log=(self.root/'server.log').open('wb')
        args=[str(self.bin/'mysqld.exe'),'--no-defaults','--basedir='+str(self.bin.parent),'--datadir='+str(self.root/'data'),
            '--bind-address=127.0.0.1','--port='+str(self.port),'--skip-name-resolve','--console','--default-time-zone=+00:00',
            '--innodb-buffer-pool-size=64M','--innodb-log-file-size=4M','--key-buffer-size=8M','--max-connections=32','--skip-log-bin']
        if self.tls:args+=['--ssl-cert='+str(self.root/'data/server-cert.pem'),'--ssl-key='+str(self.root/'data/server-key.pem')]
        self.process=subprocess.Popen(args,stdout=self.log,stderr=subprocess.STDOUT,creationflags=FLAGS)
        deadline=time.monotonic()+60
        try:
            while True:
                if self.process.poll() is not None:raise RuntimeError('Isolated SQL server exited; inspect '+str(self.root/'server.log'))
                try:
                    self.verify_owner()
                    break
                except RuntimeError:
                    if time.monotonic()>=deadline:raise RuntimeError('Isolated SQL server readiness deadline exceeded; inspect '+str(self.root/'server.log'))
                    time.sleep(0.2)
            return self
        except BaseException as exc:
            self.__exit__(type(exc),exc,exc.__traceback__);raise
    def query(self,sql):
        result=subprocess.run([str(self.bin/'mysql.exe'),'--no-defaults','--protocol=TCP','--host=127.0.0.1','--port='+str(self.port),
            '--user=root','--batch','--raw','--skip-column-names','--default-character-set=utf8mb4'],input=sql.encode(),capture_output=True,creationflags=FLAGS,timeout=30)
        if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
        return result.stdout.decode(errors='strict')
    def verify_owner(self):
        if self.process is None or self.process.poll() is not None:raise RuntimeError('Owned SQL process is not running')
        row=self.query('SELECT @@datadir, @@port;').splitlines()[0].split('\t')
        if pathlib.Path(row[0]).resolve()!=(self.root/'data').resolve() or int(row[1])!=self.port:raise RuntimeError('SQL listener does not belong to this isolated run')
        self.owned=True
    def create_schema(self,name):
        if name not in ['rs09_jvm','rs09_native']:raise ValueError('Unexpected isolated schema name')
        self.verify_owner()
        sql=(ROOT/'fixtures/sql/schema.sql').read_text(encoding='utf-8')
        sql=sql.replace('CREATE DATABASE global;','DROP DATABASE IF EXISTS '+name+'; CREATE DATABASE '+name+';').replace('USE global;','USE '+name+';')
        self.query(sql)
    def __exit__(self,exc_type,*_):
        if self.process is not None:
            if self.process.poll() is None:
                try:
                    if self.owned:
                        self.verify_owner()
                        subprocess.run([str(self.bin/'mysqladmin.exe'),'--no-defaults','--protocol=TCP','--host=127.0.0.1','--port='+str(self.port),'--user=root','shutdown'],
                            stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=FLAGS,timeout=20)
                    else:self.process.terminate()
                except (subprocess.TimeoutExpired,RuntimeError):self.process.terminate()
                try:self.process.wait(timeout=20)
                except subprocess.TimeoutExpired:self.process.terminate();self.process.wait(timeout=10)
            self.process=None
        if self.log:self.log.close();self.log=None
        if exc_type is None:
            data=(self.root/'data').resolve();workspace=(ROOT/'runs/sql-db').resolve()
            if not workspace.is_relative_to(ROOT.resolve()) or not data.is_relative_to(workspace) or data.parent!=self.root.resolve() or data.name!='data':raise RuntimeError('Refusing unexpected temporary database cleanup path')
            if data.exists():shutil.rmtree(data)
