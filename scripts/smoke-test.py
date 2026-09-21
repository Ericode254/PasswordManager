#!/usr/bin/env python3
"""Exercise the actual TUI with a fake pass executable; never touches a real vault."""
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time
import tomllib

BINARY = Path(__file__).resolve().parents[1] / "target/debug/passtui"
FAKE_PASS = r'''#!/usr/bin/env python3
import os, sys, subprocess
from pathlib import Path
root = Path(os.environ['PASSWORD_STORE_DIR'])
args = sys.argv[1:]
if args == ['--version']:
    print('mock pass'); sys.exit(0)
operation = args[0]
if operation == 'git':
    sys.exit(subprocess.call(['git', '-C', str(root), *args[1:]]))
paths = [arg for arg in args[1:] if not arg.startswith('--')]
def path(value): return root / (value + '.gpg')
if operation == 'insert':
    target = path(paths[0])
    if target.exists() and '--force' not in args: sys.exit(1)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(sys.stdin.read())
elif operation == 'show':
    sys.stdout.write(path(paths[0]).read_text())
elif operation == 'mv':
    source, target = map(path, paths)
    if target.exists(): sys.exit(1)
    target.parent.mkdir(parents=True, exist_ok=True)
    source.rename(target)
elif operation == 'rm':
    path(paths[0]).unlink()
else:
    sys.exit(1)
if operation in ('insert', 'mv', 'rm'):
    subprocess.run(['git', '-C', str(root), 'add', '--all'], check=True)
    subprocess.run(['git', '-C', str(root), 'commit', '-m', operation], check=True)
'''

with tempfile.TemporaryDirectory(prefix='passtui-smoke-') as temporary:
    root = Path(temporary)
    store = root / 'store'
    store.mkdir()
    (store / '.gpg-id').write_text('mock-key')
    mock_bin = root / 'bin'
    mock_bin.mkdir()
    executable = mock_bin / 'pass'
    executable.write_text(FAKE_PASS)
    executable.chmod(0o700)
    # Fixtures intentionally use plaintext as fake ciphertext; no real GPG keys.
    (mock_bin / 'gpg').write_text('#!/usr/bin/env python3\nimport sys\nsys.stdout.buffer.write(sys.stdin.buffer.read())\n')
    (mock_bin / 'gpg').chmod(0o700)
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 120, 0, 0))
    environment = dict(os.environ, PATH=f'{mock_bin}:{os.environ["PATH"]}',
                       PASSWORD_STORE_DIR=str(store), XDG_DATA_HOME=str(root / 'data'), XDG_CONFIG_HOME=str(root / 'config'), TERM='xterm-256color',
                       GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM='1',
                       GIT_AUTHOR_NAME='Smoke Test', GIT_AUTHOR_EMAIL='test@example.invalid',
                       GIT_COMMITTER_NAME='Smoke Test', GIT_COMMITTER_EMAIL='test@example.invalid')
    def git(*args):
        return subprocess.check_output(['git', '-C', str(store), *args], env=environment, stderr=subprocess.PIPE)
    git('init')
    git('add', '--all')
    git('commit', '-m', 'initialize fixture')
    favorite_file = root / 'data/passtui/favorites.toml'
    def favorites():
        return tomllib.loads(favorite_file.read_text())['stores'][str(store)]
    process = subprocess.Popen([str(BINARY)], stdin=slave, stdout=slave, stderr=slave, env=environment)
    os.close(slave)
    def settle(seconds=1.2):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            ready, _, _ = select.select([master], [], [], 0.03)
            if ready:
                try: os.read(master, 65536)
                except OSError: break
        assert process.poll() is None, 'TUI exited unexpectedly'
    def send(keys):
        os.write(master, keys.encode())
        settle()
    try:
        settle()
        send('agithub\tg-test-secret\talice\thttps://example.com\t\x1b[200~important note\nsecond line\x1b[201~\r')
        original = store / 'github.gpg'
        assert original.read_text() == 'g-test-secret\nusername: alice\nurl: https://example.com\nimportant note\nsecond line'
        # Recovery codes use the same encrypted entry; status updates preserve credentials.
        send('e')
        send('\t\t\t\t\x1b[200~first-recovery\nsecond-recovery\x1b[201~\r')
        assert 'recovery-code: first-recovery\nrecovery-code: second-recovery' in original.read_text()
        send('R')
        before_codes = original.read_text()
        send('x\r')
        assert original.read_text() == before_codes, 'Enter must cancel used-status change'
        send('xy')
        assert 'recovery-code-used: first-recovery' in original.read_text()
        assert 'recovery-code: second-recovery' in original.read_text()
        send('\x1b')
        send('e')
        send('X\r')
        assert 'recovery-code-used: first-recovery' in original.read_text(), 'Editing must retain used status'
        send('R')
        # Editing orders available codes before used codes; change the selected available code.
        original.write_text(original.read_text() + '\nexternal-recovery-note')
        send('xy')
        assert 'external-recovery-note' in original.read_text()
        assert 'recovery-code: second-recovery' in original.read_text(), 'Stale recovery view must not overwrite changes'
        send('\x1b')
        send('fF')
        assert favorites() == ['github']
        send('e')
        send('X\r')
        assert original.read_text().startswith('g-test-secretXX\nusername: alice')
        send('F')
        # External edits must not get overwritten by a stale form.
        send('e')
        original.write_text('externally-updated\nusername: alice')
        send('Y\r')
        assert original.read_text().startswith('externally-updated')
        send('\x1b')
        send('r')
        send('\x15Work/github.com\r')
        moved = store / 'Work/github.com.gpg'
        assert moved.exists() and not original.exists()
        assert favorites() == ['Work/github.com']
        send('/github\r')
        send('d\r')
        assert moved.exists(), 'Enter should cancel deletion'
        send('dy')
        assert not moved.exists()
        # Deleted entries remain recoverable from store history.
        before = git('rev-parse', 'HEAD')
        send('H')
        send('\r')
        send('r\r')
        assert not moved.exists(), 'Enter must cancel historical restore'
        send('ry')
        assert moved.read_text() == 'externally-updated\nusername: alice'
        assert git('rev-parse', 'HEAD') != before, 'Restore must create a new commit'
        assert before.strip() in git('rev-list', 'HEAD').splitlines(), 'Restore must preserve old history'
        # A stale preview must never overwrite a subsequent external edit.
        send('v')
        send('\r')
        moved.write_text('newer external edit')
        send('ry')
        assert moved.read_text() == 'newer external edit'
        send('\x1b')
        # Quit cleanly after completing the mutation flows.
        os.write(master, b'q')
        process.wait(timeout=3)
        assert process.returncode == 0
        os.close(master)
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 120, 0, 0))
        process = subprocess.Popen([str(BINARY), '--pick'], stdin=slave, stdout=slave, stderr=slave, env=environment)
        os.close(slave)
        settle()
        send('\x1b[200~github\x1b[201~')
        send('\x06')  # Ctrl+f toggles the persisted favorite in the picker.
        assert favorites() == []
        os.write(master, b'\x1b')
        process.wait(timeout=3)
        assert process.returncode == 0
        print('PASS: recovery add/status/edit/conflicts, multiline paste, favorites/filter/move, edit, conflict protection, deletion, history restore/cancel/stale checks')
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
