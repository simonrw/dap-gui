#!/usr/bin/env python

from __future__ import annotations
import json
import time
import socket
from typing import Any, Literal, TypedDict, cast

ServerMessageType = Literal["event", "response"]


class ServerMessage(TypedDict):
    seq: int
    type: ServerMessageType


class Response(ServerMessage):
    request_seq: int
    success: bool
    command: Literal["initialize"]
    body: Any | None


def serislise_message(msg: dict) -> bytes:
    m = json.dumps(msg)
    n = len(m)

    header = f"Content-Length: {n}"
    message_str = "\r\n".join(
        [
            header,
            "",
            m,
        ]
    )
    return message_str.encode("utf8")


def deserialise_message(body: bytes) -> ServerMessage:
    text = body.decode("utf8")
    # TODO: read content length
    header, _, body_str = text.split("\r\n", 2)
    return json.loads(body_str)


class Handler:
    def __init__(self, client: Client):
        self.client = client
        self.awaiting_response = {}

        # state about the debug adapter
        self.capabilities = {}

        # init message
        msg = {
            "command": "initialize",
            "adapter_id": "dap-gui",
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message

    def loop(self):
        while True:
            res = self.client.receive_message()
            match res["type"]:
                case "response":
                    self.handle_response(cast(Response, res))
                case "event":
                    self.handle_event(res)
                case t:
                    raise NotImplementedError(t)

    def handle_response(self, res: Response):
        req = self.awaiting_response.pop(res["request_seq"], None)
        if not req:
            raise NotImplementedError(res)

        print(f"response {res} replying to {req}")

        match res["command"]:
            case "initialize":
                self.capabilities = res["body"]

                # disconnect
                msg = {
                    "command": "disconnect",
                    "terminateDebuggee": True,
                }
                sent_message = self.client.send_request(msg)
                self.awaiting_response[sent_message["seq"]] = sent_message

            case "disconnect":
                self.client.disconnect()
                print("Got disconnect response -exiting")
                raise SystemExit(0)

    def handle_event(self, event: ServerMessage):
        print(f"Got event: {event=}")


class Client:
    def __init__(self, host: str = "127.0.0.1", port: int = 5678):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.connect((host, port))
        self.seq = 1

    def send_request(self, msg: dict) -> dict:
        full_message = {
            **msg,
            **{
                "seq": self.seq,
                "type": "request",
            },
        }
        buf = serislise_message(full_message)
        self.sock.send(buf)
        self.seq += 1
        return full_message

    def receive_message(self) -> ServerMessage:
        m = self.sock.recv(8196)
        res = deserialise_message(m)
        self.seq = res["seq"] + 1
        return res

    def disconnect(self):
        self.sock.close()


if __name__ == "__main__":
    client = Client()
    handler = Handler(client)
    handler.loop()
