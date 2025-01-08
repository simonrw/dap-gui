from contextlib import contextmanager
import copy
import io
import json
from os import PathLike

import pytest

from pythondap.session import DebugSession


PYTHON_CODE = """a = 10
b = 20
a = a + b
print(a)
"""

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


def test_execute(environment):
    with environment(PYTHON_CODE, breakpoints=[0]) as env:
        env.resume()
