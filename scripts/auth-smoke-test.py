#!/usr/bin/env python3
"""Exercise master-password setup/unlock using an isolated terminal and store."""
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

from auth_fixture import PASSWORD

BINARY = Path(__file__).resolve().parents[1] / 'target/debug/passtui'


class Session:
    def __init__(self, environment, picker=False):
        self.master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 120, 0, 0))
        self.process = subprocess.Popen([str(BINARY), *(['--pick'] if picker else [])],
                                        stdin=slave, stdout=slave, stderr=slave, env=environment)
        os.close(slave)
        self.output = b''

    def capture(self, seconds=0.5):
        data = bytearray()
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            ready, _, _ = select.select([self.master], [], [], 0.02)
            if ready:
                try:
                    data.extend(os.read(self.master, 65536))
                except OSError:
                    break
        self.output += bytes(data)
        assert PASSWORD.encode() not in self.output, 'Master password appeared in terminal output'
        return re.sub(rb'\x1b\[[0-?]*[ -/]*[@-~]', b'', bytes(data)).decode(errors='replace')

    def send(self, text, seconds=0.5):
        os.write(self.master, text.encode())
        return self.capture(seconds)

    def password(self):
        return self.send('\x1b[200~' + PASSWORD + '\x1b[201~\r', 1.5)

    def generated_password(self):
        self.send('\x07')
        output = self.redraw()
        match = re.search(r'\b[a-z]+(?:-[a-z]+){5,}\b', output)
        assert match, repr(output)
        return match.group(0)

    def redraw(self):
        # Resize to request a complete redraw, so diff rendering cannot omit
        # characters shared with the previous preview.
        size = fcntl.ioctl(self.master, termios.TIOCGWINSZ, bytes(8))
        rows, columns, _, _ = struct.unpack('HHHH', size)
        columns = 119 if columns == 120 else 120
        fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack('HHHH', rows, columns, 0, 0))
        return self.capture()

    def expect(self, output, text):
        deadline = time.monotonic() + 10
        while text.replace(' ', '') not in output.replace(' ', ''):
            assert time.monotonic() < deadline, repr(output)
            assert self.process.poll() is None, repr(output)
            output += self.capture(0.1)
        return output

    def close(self, keys='\x1b', expected=0):
        if self.process.poll() is None:
            os.write(self.master, keys.encode())
        self.process.wait(timeout=4)
        os.close(self.master)
        assert self.process.returncode == expected


with tempfile.TemporaryDirectory(prefix='passtui-auth-smoke-') as temporary:
    root = Path(temporary)
    store = root / 'store'
    store.mkdir()
    (store / '.gpg-id').write_text('mock-key')
    (store / 'private-entry-name.gpg').write_text('fake ciphertext')
    commands = root / 'bin'
    commands.mkdir()
    backend_log = root / 'backend-called'
    (commands / 'pass').write_text('#!/usr/bin/env python3\nimport os\nfrom pathlib import Path\n'
                                  'Path(os.environ["BACKEND_LOG"]).write_text("called")\n')
    (commands / 'pass').chmod(0o700)
    environment = dict(os.environ, XDG_CONFIG_HOME=str(root / 'config'),
                       XDG_DATA_HOME=str(root / 'data'), PASSWORD_STORE_DIR=str(store),
                       PATH=f'{commands}:{os.environ["PATH"]}', TERM='xterm-256color',
                       BACKEND_LOG=str(backend_log))
    record = root / 'config/passtui/master-password'
    session = Session(environment, picker=True)
    assert 'Create master password' in session.capture()
    session.close()
    assert not record.exists() and not backend_log.exists()

    session = Session(environment)
    session.capture()
    output = session.send('short\r')
    assert 'atleast15' in output.replace(' ', ''), repr(output)
    assert not record.exists() and not backend_log.exists()
    session.send('\x15')
    session.password()
    session.send('\x07')
    assert not record.exists(), 'Generating must not enroll automatically'
    session.send('\x1b')  # Cancelling must preserve the typed password.
    output = session.send('different\r')
    assert 'donotmatch' in output.replace(' ', ''), repr(output)
    assert not record.exists()
    session.password()
    output = session.expect(session.password(), 'private-entry-name')
    assert 'private-entry-name' in output, output
    assert backend_log.exists()
    assert record.read_text().startswith('$argon2id$')
    assert PASSWORD not in record.read_text()
    assert record.stat().st_mode & 0o777 == 0o600
    session.close('q')

    for picker in (False, True):
        backend_log.unlink(missing_ok=True)
        session = Session(environment, picker)
        output = session.capture()
        assert 'UnlockPassTUI' in output.replace(' ', '') and 'private-entry-name' not in output
        assert 'Generatedmasterpassword' not in session.send('\x07').replace(' ', '')
        assert not backend_log.exists()
        output = session.expect(session.send('wrong\r', 1.5), 'Incorrect master password')
        assert 'Incorrectmasterpassword' in output.replace(' ', ''), output
        assert 'private-entry-name' not in output and not backend_log.exists()
        session.capture(2.1)
        output = session.expect(session.password(), 'private-entry-name')
        assert 'private-entry-name' in output, output
        session.send('\x0c')
        output = session.expect(session.send('\r'), 'Unlock PassTUI')
        assert 'UnlockPassTUI' in output.replace(' ', ''), repr(output)
        output = session.expect(session.send('\r', 1.5), 'Incorrect master password')
        assert 'Incorrectmasterpassword' in output.replace(' ', '') and 'private-entry-name' not in output
        session.capture(2.1)
        output = session.expect(session.password(), 'private-entry-name')
        assert 'private-entry-name' in output
        session.close('\x1b' if picker else 'q')

    record.write_text('corrupted authentication record')
    backend_log.unlink(missing_ok=True)
    session = Session(environment)
    output = session.capture()
    session.close(expected=1)
    assert 'access denied' in output and 'private-entry-name' not in output
    assert not backend_log.exists()

    for picker in (False, True):
        generated_config = root / f'generated-config-{picker}'
        generated_environment = dict(environment, XDG_CONFIG_HOME=str(generated_config))
        generated_record = generated_config / 'passtui/master-password'
        session = Session(generated_environment, picker)
        session.capture()
        phrase = session.generated_password()
        # Regeneration is available without accepting the previous candidate.
        session.send('\x07')
        output = session.redraw()
        match = re.search(r'\b[a-z]+(?:-[a-z]+){5,}\b', output)
        assert match and match.group(0) != phrase, repr(output)
        phrase = match.group(0)
        output = session.send('\r')
        assert phrase not in output and not generated_record.exists()
        # Confirmation stays empty: Enter alone must not create credentials.
        session.send('\r', 1.5)
        output = session.redraw()
        assert 'donotmatch' in output.replace(' ', '') and not generated_record.exists(), repr(output)
        phrase = session.generated_password()
        session.send('\r')
        output = session.send('\x1b[200~' + phrase + '\x1b[201~\r', 1.5)
        session.expect(output, 'private-entry-name')
        assert phrase not in generated_record.read_text()
        session.close('\x1b' if picker else 'q')
        session = Session(generated_environment, picker)
        session.capture()
        output = session.send('\x1b[200~' + phrase + '\x1b[201~\r', 1.5)
        session.expect(output, 'private-entry-name')
        assert phrase.encode() not in session.output, 'Unlock input must remain masked'
        session.close('\x1b' if picker else 'q')

    print('PASS: setup/cancel, generation/regeneration/confirmation and persistence, masked input, wrong-password denial, TUI/picker startup and relock, corrupt-record denial')
