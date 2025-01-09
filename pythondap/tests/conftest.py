from contextlib import contextmanager
import copy
import io
import json
from os import PathLike
import shutil
import socket
import subprocess as sp
import sys
import threading
import time

import pytest

from pythondap.session import DebugSession


# vendored from LocalStack
def get_free_tcp_port(blocklist: list[int] | None = None) -> int:
    blocklist = blocklist or []
    for _ in range(50):
        tcp = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        tcp.bind(("", 0))
        _, port = tcp.getsockname()
        tcp.close()
        if port not in blocklist:
            return port
    raise Exception(f"Unable to determine free TCP port with blocklist {blocklist}")


@pytest.fixture(scope="session")
def compile_and_install():
    make_bin = shutil.which("make")
    if not make_bin:
        raise RuntimeError("no make command found")

    cmd = ["make", "python-develop"]
    sp.check_call(cmd)


@pytest.fixture
def port() -> int:
    return get_free_tcp_port()


class ChildMonitor(threading.Thread):
    def __init__(self, child):
        threading.Thread.__init__(self)
        self.daemon = True
        self.child = child

    def run(self):
        print("Monitoring server process")
        ret = self.child.wait()
        if ret != 0:
            raise RuntimeError(f"Child server process failed with exit code {ret}")


@pytest.fixture
def run_server(port):
    @contextmanager
    def execute(sleep_time: int = 5):
        print(f"{port=}")
        cmd = [
            sys.executable,
            "-m",
            "debugpy.adapter",
            "--host",
            "127.0.0.1",
            "--port",
            str(port),
            "--log-stderr",
        ]
        child = sp.Popen(cmd, stderr=sp.PIPE)
        assert child.stderr is not None
        monitor = ChildMonitor(child)
        monitor.start()

        for line in iter(child.stderr.readline, b""):
            line = line.decode()
            if "listening for incoming client connections" in line.lower():
                print("DAP server started up")
                break

        if sleep_time > 0:
            print("Sleeping after server startup")
            time.sleep(sleep_time)

        yield
        child.kill()

    yield execute


LAUNCH_TEMPLATE = {
    "version": "0.2.0",
    "configurations": [
        {
            "name": "PLACEHOLDER",
            "type": "debugpy",
            "request": "launch",
            "program": "PLACEHOLDER",
            "justMyCode": False,
        },
    ],
}


@pytest.fixture
def write_config():
    def inner(outfile: io.StringIO, script_path: PathLike, config_name: str):
        template = copy.deepcopy(LAUNCH_TEMPLATE)
        template["configurations"][0]["name"] = config_name
        template["configurations"][0]["program"] = str(script_path)
        json.dump(template, outfile)

    return inner


@pytest.fixture
def environment(run_server, tmp_path, write_config):
    @contextmanager
    def inner(python_code: str, breakpoints: list[int]):
        script = tmp_path.joinpath("script.py")
        with script.open("w") as outfile:
            outfile.write(python_code)

        config_path = tmp_path.joinpath("launch.json")
        config_name = "Launch"
        with config_path.open("w") as outfile:
            write_config(outfile, script, config_name)

        debugger = DebugSession(
            breakpoints=breakpoints,
            file=str(script),
            config_path=str(config_path),
            config_name=config_name,
            program=script,
        )
        yield debugger

    yield inner
