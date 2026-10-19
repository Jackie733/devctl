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
 elif mode=='completion':
  w.config=root/'.config/devctl/devctl.toml'
  projects=root/'项目目录';projects.mkdir();(projects/'space project').mkdir();(projects/'项目甲').mkdir();(projects/'项目乙').mkdir()
  (projects/'space-file').write_text('not a directory');(projects/'.hidden').mkdir()
  current=root/'unrelated-directory';current.mkdir();(current/'single').mkdir()
  ui=Tui(w,discover=True,wait_for='No workspace yet');ui.send(b'a');ui.wait('Tab: complete')
  ui.send(b'\t\r');ui.wait('command you normally run');ui.send(b'\x1b');ui.wait('Project directory');ui.send(b'\x15')
  paste(ui,'~/项目目');ui.send(b'\t');ui.wait('> ~/项目目录/_')
  paste(ui,'space');ui.send(b'\t\r');ui.wait('command you normally run')
  assert not w.config.exists(),'Completion/inspection wrote a configuration'
  ui.send(b'\x1b');ui.wait('Project directory');ui.send(b'\x15');paste(ui,projects/'项');ui.send(b'\t');ui.wait('2 directories')
  expected=sorted(['项目甲','项目乙'])
  ui.send(b'\t');ui.wait('> '+str(projects/expected[0])+'/_')
  ui.send(b'\t');ui.wait('> '+str(projects/expected[1])+'/_')
  ui.send(b'\x1b[Z');ui.wait('> '+str(projects/expected[0])+'/_')
  # Editing invalidates the candidate cache; failures keep the text editable.
  ui.send(b'\x15');paste(ui,projects/'missing');ui.send(b'\t');ui.wait('No matching directories');ui.wait('missing_')
  ui.send(b'\x15');paste(ui,'../项目目录/space');ui.send(b'\t');ui.wait('> ../项目目录/space project/_')
  # Quoted pasted paths stay literal, including spaces and Unicode.
  ui.send(b'\x15');paste(ui,'"'+str(projects/'space')+'"');ui.send(b'\t');ui.wait('space project/"_')
  ui.send(b'\r');command(ui,'sleep 60');save(ui);ui.wait('Press s to start')
  assert 'space project' in w.config.read_text() and 'Stopped' in cli('status')
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
 elif mode=='edit_remove_reload':
  empty_config();w.config.write_text(w.config.read_text().replace('enabled=false','enabled=true'))
  alpha=root/'alpha';alpha.mkdir();(alpha/'v1.sh').write_text("echo $$ >> pid\nprintf 'alpha-v1\\n'\nexec /bin/sleep 60\n");(alpha/'v2.sh').write_text("echo $$ >> pid\nprintf 'alpha-v2\\n'\nexec /bin/sleep 60\n")
  beta=root/'beta';beta.mkdir();(beta/'run.sh').write_text("echo $$ >> pid\nprintf 'beta-ready\\n'\nexec /bin/sleep 60\n")
  cli('add',str(alpha),'--name','alpha','--apply','--yes','--','/bin/sh','v1.sh')
  cli('add',str(beta),'--name','beta','--apply','--yes','--','/bin/sh','run.sh')
  ui=Tui(w);ui.wait('alpha');ui.wait('beta')
  # Start both with A, then single-restart alpha with r while beta keeps its pid.
  ui.send(b'A');ui.wait('alpha-v1');alpha_pid1=int(wait_file(alpha/'pid').splitlines()[-1]);beta_pid1=int(wait_file(beta/'pid').splitlines()[-1])
  ui.send(b'r');ui.wait('[out #3] alpha-v1')
  alpha_pid2=int(wait_file(alpha/'pid').splitlines()[-1]);assert alpha_pid2!=alpha_pid1
  assert int((beta/'pid').read_text().splitlines()[-1])==beta_pid1;os.kill(beta_pid1,0)
  # Mouse click on row 4 (1-based SGR row 4 = 0-based row 3 = beta) selects beta; wheel up selects alpha.
  ui.send(b'\x1b[<0;3;4M\x1b[<0;3;4m');ui.wait('beta-ready')
  ui.send(b'\x1b[<64;3;4M');ui.wait('alpha-v1')
  # Stop alpha, edit its command via e while beta stays running.
  ui.send(b'x');ui.wait('Selected: alpha | Stopped')
  ui.send(b'e');ui.wait('Review project update');ui.send(b'e');ui.wait('command you normally run');ui.send(b'\x15')
  paste(ui,'/bin/sh v2.sh');ui.send(b'\r');ui.wait('Review project update');save(ui)
  ui.wait('Updated alpha. Press s to start');os.kill(beta_pid1,0)
  ui.send(b's');ui.wait('alpha-v2');os.kill(beta_pid1,0)
  # Stop alpha and remove it via d + y while beta stays running.
  ui.send(b'x');ui.wait('Selected: alpha | Stopped')
  ui.send(b'd');ui.wait('Remove project');ui.send(b'y');ui.wait('Selected: beta | Running')
  assert 'alpha:' not in cli('status') and 'beta: Running' in cli('status');os.kill(beta_pid1,0)
  # Add gamma externally via config file and hot-reload with u while beta stays running.
  gamma=root/'gamma';gamma.mkdir()
  with w.config.open('a') as stream:
   stream.write(f"\n[projects.gamma]\npath = {json.dumps(str(gamma))}\n[services.gamma]\nproject = \"gamma\"\ncmd = [\"/bin/sleep\", \"60\"]\n")
  ui.send(b'u');ui.wait('gamma');assert 'gamma: Stopped' in cli('status');os.kill(beta_pid1,0)
  ui.send(b'Qy');ui.exit(key=None)

 else: raise AssertionError(mode)
 print('PASS',mode)
finally:
 for ui in w.clients:ui.close()
 cli('shutdown',ok=False)
