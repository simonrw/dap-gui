#!/usr/bin/env python

from __future__ import annotations

import asyncio
import os
import time
from threading import Event, Thread
from collections import defaultdict
import sys
from enum import Enum
from dataclasses import dataclass
import json
import socket
from typing import Any, Literal, TypedDict, cast, Generator
from queue import Queue
import structlog
import logging

logging.basicConfig(level=logging.DEBUG, filename="log.log", filemode="w", format="%(message)s")
structlog.configure(
    processors=[
        structlog.contextvars.merge_contextvars,
        structlog.processors.add_log_level,
        structlog.processors.StackInfoRenderer(),
        structlog.dev.set_exc_info,
        structlog.processors.TimeStamper(),
        structlog.processors.JSONRenderer(),
    ],
    wrapper_class=structlog.stdlib.BoundLogger,
    context_class=dict,
    logger_factory=structlog.stdlib.LoggerFactory(),
    cache_logger_on_first_use=False,
)

LOG = structlog.get_logger()


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


class ThreadStatus(Enum):
    started = "started"
    exited = "exited"


@dataclass(frozen=True)
class StackFrame:
    id: int
    name: str
    line: int
    column: int
    source: StackFrameSource

    @classmethod
    def from_raw(cls, raw: dict) -> StackFrame:
        source = StackFrameSource(path=raw["source"]["path"])
        frame = cls(
            id=raw["id"], name=raw["name"], line=raw["line"], column=raw["column"], source=source
        )
        return frame


@dataclass(frozen=True)
class StackFrameSource:
    path: str


ThreadId = int


@dataclass(frozen=True)
class Scope:
    variables_reference: int
    name: str
    expensive: bool


@dataclass(frozen=True)
class Variable:
    name: str
    value: str
    typ: str


class Handler:
    capabilities: dict
    thread_status: dict[ThreadId, ThreadStatus]
    stack_frames: dict[ThreadId, list[StackFrame] | None]
    scopes: dict[int, list[Scope]]
    variables: dict[int, list[Variable]]
    wait_event: Event | None

    def __init__(self, client: Client, events: Queue):
        self.client = client
        self.awaiting_response = {}
        self.queue = events
        self.wait_event = None

        # state about the debug adapter
        self.capabilities = {}
        self.ready = False
        self.current_thread = None
        self.thread_status = {}
        self.stack_frames = {}
        self.scopes = {}
        self.variables = defaultdict(list)

    def wait_for_connect(self):
        self.wait_event = Event()
        LOG.info("waiting for debugger to stop")
        self.wait_event.wait()
        LOG.info("debugger stopped")

    @property
    def initialised(self) -> bool:
        return len(self.capabilities) > 0

    def loop(self):
        while True:
            res = self.queue.get()
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
            return
        LOG.debug("response", type="response", response=res, request=req)

        if not res["success"]:
            LOG.warning("failed response", response=res, request=req)
            raise RuntimeError(res)

        match res["command"]:
            case "initialize":
                self.capabilities = res["body"]
                self.clear_awaiting(res)

                self.send_attach()

            case "setFunctionBreakpoints":
                self.configuration_done()

            case "threads":
                body = res["body"]
                self.stack_frames.clear()
                for thread in body["threads"]:
                    self.stack_frames[thread["id"]] = None
                    self.send_stack_trace(thread["id"])

                # TODO: wait for the results of these requests
                # self.send_continue()

            case "stackTrace":
                body = res["body"]
                stack_frames = body["stackFrames"]
                frames = []
                for raw_frame in stack_frames:
                    frame = StackFrame.from_raw(raw_frame)
                    self.send_scopes(frame)
                    frames.append(frame)

                thread_id = req["arguments"]["threadId"]
                self.stack_frames[thread_id] = frames

            case "scopes":
                body = res["body"]
                scopes = []
                for raw_scope in body["scopes"]:
                    scope = Scope(
                        variables_reference=raw_scope["variablesReference"],
                        name=raw_scope["name"],
                        expensive=raw_scope["expensive"],
                    )
                    scopes.append(scope)
                    if scope.variables_reference > 0 and not scope.expensive:
                        self.send_variables(scope.variables_reference)

                frame_id = req["arguments"]["frameId"]
                self.scopes[frame_id] = scopes
                self.clear_awaiting(res)

            case "variables":
                body = res["body"]
                for variable in body["variables"]:
                    if (ref := variable.get("variablesReference")) > 0:
                        # TODO decode further
                        return
                        self.send_variables(variable["variablesReference"])
                        self.clear_awaiting(res)
                    else:
                        v = Variable(
                            name=variable["name"],
                            value=variable["value"],
                            typ=variable["type"],
                        )
                        variable_ref = req["arguments"]["variablesReference"]
                        # if variable_ref in self.variables:
                        #     raise ValueError(f"Already encountered variable {variable_ref}: {v}")
                        self.variables[variable_ref].append(v)
                        LOG.debug(
                            "variables",
                            count=sum(len(self.variables[key]) for key in self.variables),
                        )
                    self.clear_awaiting(res)

            case "disconnect":
                self.client.disconnect()
                LOG.debug("disconnect")
                raise SystemExit(0)

    def clear_awaiting(self, response: Response):
        self.awaiting_response.pop(response["request_seq"], None)

    def handle_event(self, event: ServerMessage):
        LOG.debug("event", message=event)

        match event["event"]:
            case "initialized":
                self.initialized = True
                self.set_breakpoints()
                self.set_function_breakpoints()
            case "stopped":
                if self.wait_event and not self.wait_event.is_set():
                    self.wait_event.set()

                self.current_thread = event["body"]["threadId"]
                self.reset_state()
                self.send_threads()
            case "output":
                body = event["body"]
                match body["category"]:
                    case "stdout":
                        print(body["output"])
                    case "stderr":
                        print(body["output"], file=sys.stderr)
            case "thread":
                body = event["body"]
                thread_id = body["threadId"]
                status = ThreadStatus(body["reason"])
                self.thread_status[thread_id] = status
                LOG.debug("thread", status=self.thread_status)
            case "terminated":
                self.disconnect()

    def reset_state(self):
        self.stack_frames.clear()
        self.scopes.clear()

    def send_initialize(self):
        msg = {
            "command": "initialize",
            "arguments": {
                "adapterID": "dap-gui",
                "clientName": "DAP GUI",
                "pathFormat": "path",
                # TODO
                "supportsRunInTerminalRequest": False,
                "supportsStartDebuggingRequest": False,
            },
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message

    def send_disconnect(self):
        msg = {
            "command": "disconnect",
            "terminateDebuggee": True,
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message

    def send_variables(self, variables_reference: int):
        msg = {
            "command": "variables",
            "arguments": {"variablesReference": variables_reference},
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message

    def send_stack_trace(self, thread_id: ThreadId):
        msg = {
            "command": "stackTrace",
            "arguments": {
                "threadId": thread_id,
            },
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message

    def send_scopes(self, frame: StackFrame):
        msg = {
            "command": "scopes",
            "arguments": {
                "frameId": frame.id,
            },
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message

    def configuration_done(self):
        msg = {
            "command": "configurationDone",
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message

    def send_threads(self):
        msg = {
            "command": "threads",
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message

    def send_attach(self):
        msg = {
            "command": "attach",
            "arguments": {
                "name": "Python: Remote Attach (ext)",
                "type": "python",
                "request": "attach",
                "connect": {"host": "localhost", "port": 5678},
                "pathMappings": [
                    {
                        "localRoot": "/home/simon/work/localstack/localstack-ext/localstack_ext",
                        "remoteRoot": "/opt/code/localstack/.venv/lib/python3.10/site-packages/localstack_ext",
                    }
                ],
                "justMyCode": False,
                "__configurationTarget": 6,
                "clientOS": "unix",
                "debugOptions": ["DebugStdLib", "RedirectOutput", "ShowReturnValue"],
                "showReturnValue": True,
                "workspaceFolder": "/home/simon/work/localstack/localstack-ext",
                "__sessionId": "e4998b67-23f8-4ab0-adee-a14556f1ce01",
            },
            "type": "request",
            "seq": 2,
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

    def set_breakpoints(self):
        msg = {
            "command": "setBreakpoints",
            "arguments": {
                "source": {
                    "name": "mapping.py",
                    "path": "/home/simon/work/localstack/localstack-ext/localstack_ext/services/appsync/mapping.py",
                },
                "lines": [114],
                "breakpoints": [{"line": 114}],
                "sourceModified": False,
            },
        }
        sent_message = self.client.send_request(msg)
        self.awaiting_response[sent_message["seq"]] = sent_message

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
        LOG.debug("request", request=full_message)
        buf = serislise_message(full_message)
        self.sock.send(buf)
        self.seq += 1
        return full_message

    def receive_message(self) -> Generator[ServerMessage, None, None]:
        try:
            m = self.sock.recv(8196).decode("utf8")
        except OSError:
            LOG.warning("nothing read")
            # EOF
            raise SystemExit(0)

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

            yield res

    def disconnect(self):
        self.sock.close()


def receive_messages(client: Client, events: Queue):
    try:
        while True:
            for msg in client.receive_message():
                events.put(msg)
    except Exception as e:
        LOG.warning("background thread error", exc_info=e)


if __name__ == "__main__":
    queue = Queue()
    client = Client(port=5678)
    message_handler = Thread(
        target=receive_messages,
        name="message-handler",
        kwargs=dict(client=client, events=queue),
        daemon=True,
    )
    message_handler.start()

    handler = Handler(client, queue)
    ht = Thread(target=handler.loop, name="handler-loop", daemon=True)
    ht.start()

    handler.send_initialize()
    handler.wait_for_connect()
    breakpoint()
    handler.send_continue()
    time.sleep(10)
    handler.send_disconnect()
