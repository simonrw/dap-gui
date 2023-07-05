#!/usr/bin/env python

import socket
import json

body = json.dumps({
    "seq": 1,
    "type": "request",
    "command": "initialize",
    "adapterID": "dap-gui",
    })
body_len = len(body.encode("utf8"))
msg = f"Content-Length: {body_len}\r\n\r\n{body}"
print(msg)


s = socket.socket()
s.connect(("127.0.0.1", 5678))

s.send(msg.encode("utf8"))

while True:
    msg = s.recv(1024)
    print(msg.decode())


