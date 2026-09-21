#!/usr/bin/env python3
"""Exercise privacy lock and TOTP using isolated fake pass/oathtool programs."""
import fcntl
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import tempfile
import termios
import time

BINARY = Path(__file__).resolve().parents[1] / 'target/debug/passtui'
FAKE_PASS = '''#!/usr/bin/env python3
import os, sys, time
from pathlib import Path
if sys.argv[1:] == ['--version']: print('fake pass'); sys.exit(0)
if sys.argv[1] == 'show':
    if sys.argv[2] == 'slow': time.sleep(1.0)
    print((Path(os.environ['PASSWORD_STORE_DIR']) / (sys.argv[2] + '.gpg')).read_text(), end='')
else: sys.exit(1)
'''
FAKE_OATH = '''#!/usr/bin/env python3
import os, sys
from pathlib import Path
args = sys.argv[1:]
if args == ['--version']: print('mock oathtool'); sys.exit(0)
assert args[-1] == '-'
assert '--base32' in args and '--totp=SHA256' in args and '--digits=8' in args and '--time-step-size=30s' in args
assert not any('GEZDGNBV' in arg for arg in args)
assert sys.stdin.read().strip() == 'GEZDGNBV'
Path(os.environ['OATH_CALLED']).write_text('called')
print('12345678')
'''


def plain(data):
    return re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', data).decode(errors='replace')


with tempfile.TemporaryDirectory(prefix='passtui-security-') as temporary:
    root = Path(temporary)
    store = root / 'store'
    store.mkdir()
    (store / '.gpg-id').write_text('test-key')
    uri = 'otpauth://totp/Test?secret=GEZDGNBV&algorithm=SHA256&digits=8&period=30'
    (store / 'login.gpg').write_text('hidden-password\nusername: alice\n' + uri)
    (store / 'hotp.gpg').write_text('otpauth://hotp/Test?secret=GEZDGNBV&counter=0')
    (store / 'slow.gpg').write_text('late-secret')
    commands = root / 'bin'
    commands.mkdir()
    for name, content in [('pass', FAKE_PASS), ('oathtool', FAKE_OATH)]:
        executable = commands / name
        executable.write_text(content)
        executable.chmod(0o700)
    config = root / 'config/passtui'
    config.mkdir(parents=True)
    environment = dict(os.environ, PATH=f'{commands}:{os.environ["PATH"]}',
                       PASSWORD_STORE_DIR=str(store), XDG_CONFIG_HOME=str(root / 'config'),
                       XDG_DATA_HOME=str(root / 'data'), OATH_CALLED=str(root / 'called'), TERM='xterm-256color')

    # Mock every supported package manager and sudo. Tests never install system packages.
    fixture = root / 'oath-fixture'
    fixture.write_text(FAKE_OATH)
    environment['OATH_FIXTURE'] = str(fixture)
    environment['INSTALL_LOG'] = str(root / 'install-log')
    environment['MOCK_BIN'] = str(commands)
    installer = """#!/usr/bin/env python3
import os, sys, shutil
from pathlib import Path
assert sys.argv[-1] in ('oath-toolkit', 'oathtool')
target = Path(os.environ['MOCK_BIN']) / 'oathtool'
shutil.copyfile(os.environ['OATH_FIXTURE'], target)
target.chmod(0o700)
with open(os.environ['INSTALL_LOG'], 'a') as log: log.write('installed\\n')
"""
    for program in ['pacman', 'apt-get', 'dnf', 'brew']:
        (commands / program).write_text(installer)
        (commands / program).chmod(0o700)
    (commands / 'sudo').write_text('#!/usr/bin/env python3\nimport os, sys\nassert sys.argv[1] == "--"\nos.execvp(sys.argv[2], sys.argv[2:])\n')
    (commands / 'id').write_text('#!/usr/bin/env python3\nprint("1000")\n')
    for program in ['sudo', 'id']: (commands / program).chmod(0o700)
    (commands / 'oathtool').write_text('#!/usr/bin/env python3\nraise SystemExit(1)\n')
    setup = subprocess.run([str(BINARY), '--install-otp'], env=environment, capture_output=True, text=True)
    assert setup.returncode == 0, setup.stderr
    assert (root / 'install-log').read_text() == 'installed\n'
    setup = subprocess.run([str(BINARY), '--install-otp'], env=environment, capture_output=True, text=True)
    assert setup.returncode == 0 and 'already installed' in setup.stdout
    assert (root / 'install-log').read_text() == 'installed\n', 'Existing dependencies must not reinstall'

    def launch(picker=False, timeout=0):
        (config / 'config.toml').write_text(f'[behavior]\nidle_lock_seconds = {timeout}\n')
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 120, 0, 0))
        process = subprocess.Popen([str(BINARY), *(['--pick'] if picker else [])], stdin=slave, stdout=slave, stderr=slave, env=environment)
        os.close(slave)
        return master, process

    def capture(seconds=0.4):
        data = bytearray()
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            ready, _, _ = select.select([master], [], [], 0.02)
            if ready:
                try: data.extend(os.read(master, 65536))
                except OSError: break
        assert process.poll() is None, f'Unexpected exit: {plain(data)}'
        return plain(data)

    def send(text, seconds=0.4):
        os.write(master, text.encode())
        return capture(seconds)

    def close(keys):
        os.write(master, keys.encode())
        process.wait(timeout=3)
        assert process.returncode == 0
        os.close(master)

    master, process = launch()
    try:
        capture()
        send('/login\r')
        details = send('\r')
        assert 'GEZDGNBV' not in details and 'hidden-password' not in details
        (commands / 'oathtool').write_text('#!/usr/bin/env python3\nraise SystemExit(1)\n')
        output = send('t', 0.4)
        assert 'InstallOTPsupport' in output.replace(' ', ''), output
        output = send('i', 0.9)
        assert (root / 'install-log').read_text() == 'installed\ninstalled\n'

        assert '12345678' in output and 'Expiresin' in output.replace(' ', ''), output
        assert (root / 'called').exists()
        assert 'GEZDGNBV' not in output
        assert 'Sessioncleared' in send('\x0c').replace(' ', '')
        assert '12345678' not in send('t')
        send('\r')
        send('/hotp\r')
        (root / 'called').unlink()
        assert 'OnlyTOTP' in send('t', 0.6).replace(' ', '')
        assert not (root / 'called').exists(), 'HOTP must never invoke generator'
        send('\x1b')
        send('/slow\r')
        send('e', 0.1)
        assert 'Sessioncleared' in send('\x0c', 0.2).replace(' ', '')
        send('\r', 0.1)  # Cannot resume until pending decryption has completed.
        assert 'late-secret' not in capture(1.0)
        send('\r')
        close('q')
    finally:
        if process.poll() is None: process.kill(); process.wait(); os.close(master)

    for picker in (False, True):
        master, process = launch(picker, timeout=1)
        try:
            capture(0.2)
            output = send('search' if picker else 'adraft\tdraft-password', 0.2)
            deadline = time.monotonic() + 8
            while 'Sessioncleared' not in output.replace(' ', '') and time.monotonic() < deadline:
                output += capture(0.3)
            assert 'Sessioncleared' in output.replace(' ', ''), output
            assert 'draft-password' not in output
            send('\r', 0.1)
            close('\x1b' if picker else 'q')
        finally:
            if process.poll() is None: process.kill(); process.wait(); os.close(master)
    print('PASS: automatic OTP setup/return/idempotency, TOTP parameters/stdin/countdown, seed masking, HOTP rejection, late-worker/manual lock, idle lock in TUI and picker')
