import socket
import subprocess as sp
import shutil
import sys
import time

import pytest


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


@pytest.fixture(scope="session", autouse=True)
def compile_and_install():
    make_bin = shutil.which("make")
    if not make_bin:
        raise RuntimeError("no make command found")

    cmd = ["make", "python-develop"]
    sp.check_call(cmd)


@pytest.fixture
def port() -> int:
    return get_free_tcp_port()


@pytest.fixture(autouse=True)
def run_server(port):
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
    for line in iter(child.stderr.readline, b""):
        line = line.decode()
        if "Listening for incoming Client connections" in line:
            print("DAP server started up")
            break

    print("Sleeping after server startup")
    time.sleep(5)
    yield
    child.kill()
