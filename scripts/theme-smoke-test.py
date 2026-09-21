#!/usr/bin/env python3
"""Verify custom theme rendering in an isolated TUI and picker (no real vault)."""
import fcntl
import os
from pathlib import Path
import pty
import select
import shutil
import struct
import subprocess
import tempfile
import termios
import time

PROJECT = Path(__file__).resolve().parents[1]
BINARY = PROJECT / 'target/debug/passtui'


def capture(master, seconds=0.6):
    output = bytearray()
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        ready, _, _ = select.select([master], [], [], 0.03)
        if ready:
            try:
                output.extend(os.read(master, 65536))
            except OSError:
                break
    return bytes(output)


with tempfile.TemporaryDirectory(prefix='passtui-theme-smoke-') as temporary:
    root = Path(temporary)
    config = root / 'config/passtui'
    (config / 'themes').mkdir(parents=True)
    shutil.copyfile(PROJECT / 'themes/my-theme.toml', config / 'themes/my-theme.toml')
    store = root / 'store'
    store.mkdir()
    (store / '.gpg-id').write_text('mock-key')
    commands = root / 'bin'
    commands.mkdir()
    (commands / 'pass').write_text('#!/bin/sh\nexit 0\n')
    (commands / 'pass').chmod(0o700)
    environment = dict(os.environ, XDG_CONFIG_HOME=str(root / 'config'),
                       PASSWORD_STORE_DIR=str(store), XDG_DATA_HOME=str(root / 'data'), PATH=f'{commands}:{os.environ["PATH"]}',
                       TERM='xterm-256color', COLORTERM='truecolor')
    # The agent environment disables colors; this test explicitly exercises RGB output.
    environment.pop('NO_COLOR', None)
    for picker in (False, True):
        for setting, expected in [("name = 'my-theme'", b'48;2;16;24;39'),
                                  ("file = 'themes/my-theme.toml'", b'48;2;16;24;39'),
                                  ("name = 'missing-theme'", b'48;2;30;30;46')]:
            (config / 'config.toml').write_text(f'[theme]\n{setting}\n')
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 30, 120, 0, 0))
            process = subprocess.Popen([str(BINARY), *(['--pick'] if picker else [])],
                                       stdin=slave, stdout=slave, stderr=slave, env=environment)
            os.close(slave)
            try:
                output = capture(master)
                assert process.poll() is None, 'UI exited unexpectedly'
                assert expected in output, f'Wrong background: picker={picker}, {setting}'
                if not picker and 'missing' not in setting:
                    os.write(master, b'a')
                    assert expected in capture(master), 'Dialog did not retain custom background'
                os.write(master, b'\x1b')
                capture(master, 0.2)
                if not picker:
                    os.write(master, b'q')
                process.wait(timeout=3)
                assert process.returncode == 0
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait()
                os.close(master)
    print('PASS: custom name/file selection, dialog backgrounds, and missing-theme fallback in TUI and picker')
