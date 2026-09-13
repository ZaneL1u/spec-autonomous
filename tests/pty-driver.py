"""Pipe a CLI through a real POSIX terminal for keyboard-level E2E tests."""
import errno
import os
import pty
import select
import signal
import sys

pid, terminal = pty.fork()
if pid == 0:
    os.execvp(sys.argv[1], sys.argv[1:])

def stop(*_):
    raise KeyboardInterrupt

signal.signal(signal.SIGTERM, stop)
inputs = [terminal, 0]
try:
    while True:
        ready, _, _ = select.select(inputs, [], [])
        if terminal in ready:
            try:
                data = os.read(terminal, 65536)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            if not data:
                break
            os.write(1, data)
        if 0 in ready:
            data = os.read(0, 4096)
            if not data:
                inputs.remove(0)
                data = b"\x04"
            os.write(terminal, data)
    _, status = os.waitpid(pid, 0)
    sys.exit(os.waitstatus_to_exitcode(status))
finally:
    os.close(terminal)
    try:
        os.killpg(pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
