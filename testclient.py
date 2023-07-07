#!/usr/bin/env python

from __future__ import annotations
import os
from dataclasses import dataclass
import time
import json
import socket
from typing import Any, Literal, TypedDict, cast, Generator

ServerMessageType = Literal["event", "response"]


class ServerMessage(TypedDict):
    seq: int
    type: ServerMessageType


class Response(ServerMessage):
    request_seq: int
    success: bool
    command: Literal["initialize"]
    body: Any | None


@dataclass
class Header:
    content_length: int


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


class Handler:
    capabilities: dict

    def __init__(self, client: Client):
        self.client = client
        self.awaiting_response = {}

        # state about the debug adapter
        self.capabilities = {}
        self.ready = False
        self.current_thread = None

        # init message
        msg = {
            "command": "initialize",
            "arguments": {
                "adapterID": "dap-gui",
                "clientName": "DAP GUI",
                "pathFormat": "path",
                # TODO
                "supportsRunInTerminalRequest": False,
            },
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message

    @property
    def initialised(self) -> bool:
        return len(self.capabilities) > 0

    def loop(self):
        while True:
            for res in self.client.receive_message():
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
        print(f"RESPONSE: {res} replying to {req}")

        match res["command"]:
            case "initialize":
                self.capabilities = res["body"]
                print(f"Server cababilities: {json.dumps(self.capabilities, indent=2)}")

                # launch the debugee
                self.launch()
            case "setFunctionBreakpoints":
                self.configuration_done()

            case "disconnect":
                self.client.disconnect()
                print("Got disconnect response -exiting")
                raise SystemExit(0)

    def handle_event(self, event: ServerMessage):
        print(f"EVENT: {event=}")

        match event["event"]:
            case "initialized":
                self.initialized = True
                self.set_function_breakpoints()
            case "stopped":
                self.current_thread = event["body"]["threadId"]
                self.send_continue()

    def disconnect(self):
        msg = {
            "command": "disconnect",
            "terminateDebuggee": True,
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message

    def configuration_done(self):
        msg = {
            "command": "configurationDone",
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message

    def launch(self):
        msg = {
            "command": "launch",
            "arguments": {
                "program": os.path.join(os.getcwd(), "test.py"),
            },
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message

    def send_continue(self):
        msg = {
            "command": "continue",
            "arguments": {
                "threadId": self.current_thread,
            },
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message
        self.current_thread = None

    def set_function_breakpoints(self):
        msg = {
            "command": "setFunctionBreakpoints",
            "arguments": {
                "breakpoints": [
                    {"name": "main"},
                ],
            },
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message


def parse_header(header_text: str) -> Header:
    key, value = header_text.split(":")
    assert key.strip() == "Content-Length"
    return Header(content_length=int(value.strip()))


class Client:
    def __init__(self, host: str = "127.0.0.1", port: int = 5678):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.connect((host, port))
        self.seq = 1
        self.buf = ""

    def send_request(self, msg: dict) -> dict:
        full_message = {
            **msg,
            **{
                "seq": self.seq,
                "type": "request",
            },
        }
        print(f"Sending message: {full_message}")
        buf = serislise_message(full_message)
        self.sock.send(buf)
        self.seq += 1
        return full_message

    def receive_message(self) -> Generator[ServerMessage, None, None]:
        m = self.sock.recv(8196).decode("utf8")
        self.buf += m
        # TODO: what if more headers are added?
        assert self.buf.startswith("Content-Length")

        while True:
            # try to read a single message
            try:
                header_str, _, rest = self.buf.split("\r\n", 2)
            except ValueError:
                break

            header = parse_header(header_str)
            if header.content_length <= 0:
                raise RuntimeError(f"Invalid content length read: {header=}")

            if len(rest) < header.content_length:
                # not enough data in the buffer so receive again
                break

            body_str = rest[: header.content_length]
            self.buf = rest[header.content_length :]

            try:
                res = json.loads(body_str)
            except json.JSONDecodeError as e:
                raise RuntimeError("could not read message body") from e

            self.seq = res["seq"] + 1
            yield res

    def disconnect(self):
        self.sock.close()


def main():
    # marker file to prove the debuggee was run
    try:
        os.remove("out.txt")
    except:
        pass

    client = Client()
    handler = Handler(client)
    handler.loop()


if __name__ == "__main__":
    main()
