"""Real-terminal onboarding checks; no project tools are installed or probed."""
import json, os, pathlib, subprocess, sys, tempfile, time, socket, struct, re
from tui_pty import Tui
binary, mode = sys.argv[1:]
binary=str(pathlib.Path(binary).resolve())
root=pathlib.Path(tempfile.mkdtemp(prefix='dta-',dir='/tmp'))
class Workspace: pass
w=Workspace();w.root=root;w.config=root/'workspace.toml';w.clients=[]
env=os.environ.copy();env['XDG_STATE_HOME']=str(root/'state')
def cli(*args, ok=True):
 p=subprocess.run([binary,'--config',str(w.config),*args],env=env,cwd=root,capture_output=True,text=True,timeout=15)
 if ok: assert p.returncode==0,(args,p.stdout,p.stderr)
 return p.stdout

def control(action, fields=''):
 def read_exact(stream, size):
  data=b''
  while len(data)<size:
   block=stream.recv(size-len(data));assert block,'Unexpected control EOF';data+=block
  return data
 with socket.socket(socket.AF_UNIX) as stream:
  stream.settimeout(10);stream.connect(str(w.config)+'.devctl/control.sock');stream.sendall(b'CONTROL1\n')
  assert read_exact(stream,9)==b'DEVCTL 1\n'
  read_exact(stream,struct.unpack('!I',read_exact(stream,4))[0])
  payload=("action="+json.dumps(action)+"\n"+fields).encode();stream.sendall(struct.pack('!I',len(payload))+payload)
  response=read_exact(stream,struct.unpack('!I',read_exact(stream,4))[0]).decode()
  assert not re.search(r'^error =',response,re.M),response
  return response

def workspace_id(): return re.search(r'^workspace_id = "([^"]+)"',control('status'),re.M).group(1)

def wait_file(path):
 deadline=time.monotonic()+8
 while not path.exists() or not path.read_text().strip():
  assert time.monotonic()<deadline,path
  time.sleep(.02)
 return path.read_text()

def paste(ui,text): ui.send(b'\x1b[200~'+str(text).encode()+b'\x1b[201~')
def enter_path(ui,path):
 ui.send(b'a');ui.wait('Project directory');paste(ui,path);ui.send(b'\r')
def command(ui,text):
 ui.wait('command you normally run');paste(ui,text);ui.send(b'\r');ui.wait('Review project addition')
def save(ui):
 # At this terminal size the whole preview fits. A long error may require scrolling.
 ui.send(b'\r')

def empty_config(): w.config.write_text("version=1\n[workspace]\nname='existing'\n[logs]\nenabled=false\n")
try:
 if mode in ['manual','default','cancel','small']:
  if mode=='default': w.config=root/'.config/devctl/devctl.toml'
  project=root/'python q project';project.mkdir();(project/'pyproject.toml').write_text('unused type marker')
  script=project/'server with spaces.sh';script.write_text("echo $$ > pid\nprintf 'added-ready\\n'\nexec /bin/sleep 60\n")
  ui=Tui(w,discover=(mode=='default'),wait_for='No workspace yet')
  ui.send(b'a');ui.wait('Project directory');paste(ui,root/'MISSING');ui.send(b'\r');ui.wait('Cannot open directory')
  ui.send(b'\x15');paste(ui,project);ui.send(b'\r');ui.wait('Python')
  command(ui,"/bin/sh 'server with spaces.sh'")
  ui.send(b'e');ui.wait('command you normally run');ui.send(b'\x15')
  paste(ui,"/bin/sh 'server with spaces.sh'");ui.send(b'\r');ui.wait('Review project addition')
  if mode=='cancel':
   ui.send(b'\x1b');ui.wait('Project directory');ui.send(b'\x1b');ui.wait(lambda text: 'Project directory' not in text and 'No workspace yet' in text);ui.exit()
   assert not w.config.exists()
  else:
   if mode=='small':
    ui.resize(30,7);ui.wait('Window too small');ui.send(b'\r');ui.pump(.15);assert not w.config.exists()
    ui.resize(45,11);ui.wait('Resize to at least');ui.send(b'\r');ui.pump(.1);assert not w.config.exists()
    ui.resize(120,30);ui.wait('Review project addition')
   save(ui);ui.wait('Press s to start');assert w.config.exists();assert not (project/'pid').exists()
   source=w.config.read_bytes();assert b'node_environment' not in source
   ui.send(b's');ui.wait('added-ready');pid=int(wait_file(project/'pid'))
   ui.exit();ui=Tui(w);ui.wait('added-ready');assert int((project/'pid').read_text())==pid
   ui.send(b'x');ui.wait('Stopped');ui.send(b'Qy');ui.exit(key=None)
   # A duplicate must preserve user comments, bytes, inode and the original command.
   with w.config.open('a') as stream: stream.write('# preserve my comment\n')
   source=w.config.read_bytes();inode=w.config.stat().st_ino
   ui=Tui(w);enter_path(ui,project);command(ui,'false');ui.wait('Already configured')
   save(ui);ui.wait('existing command and edits preserved');assert source==w.config.read_bytes() and inode==w.config.stat().st_ino
   ui.send(b'Qy');ui.exit(key=None)
 elif mode=='workspace':
  empty_config();project=root/'repo';(project/'apps/api').mkdir(parents=True);(project/'apps/web').mkdir()
  (project/'package.json').write_text(json.dumps({'packageManager':'pnpm@999.0.0','workspaces':['apps/*'],'scripts':{'dev':'DO_NOT_EXECUTE_ROOT'}}))
  (project/'apps/api/package.json').write_text(json.dumps({'scripts':{'dev':'DO_NOT_EXECUTE_API','dev:inspect':'DO_NOT_EXECUTE_INSPECT'}}))
  (project/'apps/web/package.json').write_text(json.dumps({'scripts':{'dev':'DO_NOT_EXECUTE_WEB'}}))
  ui=Tui(w);enter_path(ui,project);ui.wait('Choose a project (root scripts');ui.send(b'j\r');ui.wait('Choose a declared script');ui.send(b'j\r');ui.wait('Review project addition');ui.wait('dev:inspect')
  save(ui);ui.wait('Press s to start');text=w.config.read_text();assert 'dev:inspect' in text and '999.0.0' not in text and 'apps/api' in text and 'apps/web' not in text
  assert 'api: Stopped' in cli('status');ui.send(b'Qy');ui.exit(key=None)
 elif mode in ['source_changed','target_changed']:
  empty_config();original=w.config.read_bytes();project=root/'node';project.mkdir()
  manifest=project/'package.json';manifest.write_text(json.dumps({'packageManager':'npm@11.0.0','scripts':{'dev':'MUST_NOT_RUN'}}))
  ui=Tui(w);enter_path(ui,project);ui.wait('Review project addition')
  if mode=='source_changed': manifest.write_text(json.dumps({'packageManager':'npm@11.0.0','scripts':{'dev':'CHANGED'}}))
  else: w.config.write_text("version=1\n[workspace]\nname='external editor'\n[logs]\nenabled=false\n")
  save(ui);ui.wait('changed');assert (w.config.read_bytes()==original) if mode=='source_changed' else ('external editor' in w.config.read_text())
  ui.exit(key=b'\x03')
 elif mode=='concurrent':
  empty_config();first=root/'first';first.mkdir();second=root/'second';second.mkdir()
  ui=Tui(w);other=Tui(w);identity=workspace_id()
  enter_path(ui,first);command(ui,'sleep 60');enter_path(other,second);command(other,'sleep 60')
  save(ui);ui.wait('Press s to start');saved=w.config.read_bytes()
  save(other);other.wait('changed');assert saved==w.config.read_bytes() and workspace_id()==identity
  assert 'first: Stopped' in cli('status') and 'second:' not in cli('status')
  other.exit(key=b'\x03');ui.send(b'Qy');ui.exit(key=None)
 elif mode=='running':
  empty_config();w.config.write_text(w.config.read_text().replace('enabled=false','enabled=true'))
  survivor=root/'survivor';survivor.mkdir();script=survivor/'server.sh';script.write_text("echo $$ > pid\nprintf 'survivor-ready\\n'\nexec /bin/sleep 60\n")
  cli('add',str(survivor),'--name','survivor','--apply','--yes','--','/bin/sh',str(script));original=w.config.read_bytes()
  project=root/'new-project';project.mkdir()
  script2=project/'server.sh';script2.write_text("echo $$ > pid\nprintf 'new-project-ready\\n'\nexec /bin/sleep 60\n")
  observer=Tui(w);identity=workspace_id()
  ui=Tui(w);enter_path(ui,project);command(ui,'/bin/sh server.sh')
  # Another client starts a service after the configuration preview was prepared.
  cli('start','survivor');pid=int(wait_file(survivor/'pid'))
  observer.wait('survivor-ready')
  before=control('logs','service="survivor"\nlimit=10\n');assert 'survivor-ready' in before
  save(ui);ui.wait('Press s to start');assert workspace_id()==identity
  observer.wait('new-project');assert w.config.read_bytes()!=original;os.kill(pid,0)
  assert 'survivor: Running' in cli('status') and 'new-project: Stopped' in cli('status')
  assert 'survivor-ready' not in ui.screen.text(),'Old project logs leaked into new selection'
  assert before==control('logs','service="survivor"\nlimit=10\n'),'Old log sequence/instance/session changed'
  ui.send(b's');ui.wait('new-project-ready');newpid=int(wait_file(project/'pid'));os.kill(pid,0)
  assert 'new-project-ready' in cli('logs','new-project','--limit','10')
  ui.exit();ui=Tui(w);ui.wait('new-project-ready');assert workspace_id()==identity
  assert int((project/'pid').read_text())==newpid;os.kill(pid,0)
  cli('stop','new-project');os.kill(pid,0);ui.wait('Stopped')
  ui.send(b'Qy');ui.exit(key=None);observer.exit()
  assert 'new-project-ready' in cli('logs','new-project','--limit','10'),'New log stream was not archived'

 else: raise AssertionError(mode)
 print('PASS',mode)
finally:
 for ui in w.clients:ui.close()
 cli('shutdown',ok=False)
