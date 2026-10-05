"""Enter a test-only master password through the real terminal prompt."""
import os
import re
import time

PASSWORD = 'orchid-lantern-quartz-voyage-cobalt-finch'


def authenticate(master, capture, setup=False):
    entry = '\x1b[200~' + PASSWORD + '\x1b[201~\r'
    os.write(master, (entry * (2 if setup else 1)).encode())
    deadline = time.monotonic() + 10
    output = ''
    while time.monotonic() < deadline:
        chunk = capture(0.1)
        if chunk is None:  # Store smoke test only drains terminal output.
            capture(1.5)
            return None
        output += chunk.decode(errors='replace') if isinstance(chunk, bytes) else chunk
        plain = re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]', '', output).replace(' ', '')
        if 'PasswordStore' in plain or 'PassTUIPick' in plain:
            return output
    raise AssertionError('Authentication did not reach the store interface')
