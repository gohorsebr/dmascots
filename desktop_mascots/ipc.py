"""Talk to the running instance over a per-user local socket (Unix socket / Windows named pipe).

Protocol: one JSON object per line in each direction.
    {"cmd": "say", "text": "...", "to": "all" | "<mascot>", "seconds": float | null}
    {"cmd": "spawn", "mascots": ["monkey", ...], "lifetime": float | null, "say": str | null}
    {"cmd": "quit"}
Replies: {"ok": true, "count": N} or {"ok": false, "error": "..."}
"""
from __future__ import annotations

import getpass
import json
import re
from typing import Callable

from PySide6.QtCore import QObject
from PySide6.QtNetwork import QLocalServer, QLocalSocket

SERVER_NAME = "desktop-mascots-" + re.sub(r"[^A-Za-z0-9_.-]", "_", getpass.getuser())


def send(message: dict, timeout_ms: int = 1500) -> dict | None:
    """Send one request to the running instance. None if nothing is listening.

    Needs a Q(Core)Application to exist (the caller creates it).
    """
    sock = QLocalSocket()
    sock.connectToServer(SERVER_NAME)
    if not sock.waitForConnected(timeout_ms):
        return None
    sock.write((json.dumps(message) + "\n").encode())
    sock.waitForBytesWritten(timeout_ms)
    while not sock.canReadLine():
        if not sock.waitForReadyRead(timeout_ms):
            return {"ok": False, "error": "no reply from the running instance"}
    reply = json.loads(bytes(sock.readLine()).decode())
    sock.disconnectFromServer()
    return reply


class Server(QObject):
    """Listens for requests and answers each line with handler(request)."""

    def __init__(self, handler: Callable[[dict], dict], parent: QObject | None = None):
        super().__init__(parent)
        self.handler = handler
        self.server = QLocalServer(self)
        self.server.setSocketOptions(QLocalServer.UserAccessOption)  # only this user may connect
        QLocalServer.removeServer(SERVER_NAME)  # clear a stale socket left by a crash
        self.listening = self.server.listen(SERVER_NAME)
        self.server.newConnection.connect(self._accept)

    def _accept(self) -> None:
        while (sock := self.server.nextPendingConnection()) is not None:
            sock.readyRead.connect(lambda s=sock: self._read(s))
            sock.disconnected.connect(sock.deleteLater)

    def _read(self, sock: QLocalSocket) -> None:
        while sock.canReadLine():
            try:
                request = json.loads(bytes(sock.readLine()).decode("utf-8", "replace"))
                reply = self.handler(request) if isinstance(request, dict) else {"ok": False, "error": "bad request"}
            except Exception as e:  # never let a bad client take the mascots down
                reply = {"ok": False, "error": str(e)}
            sock.write((json.dumps(reply) + "\n").encode())
            sock.flush()
