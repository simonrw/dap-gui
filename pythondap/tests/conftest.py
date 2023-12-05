import subprocess as sp

import pytest


@pytest.fixture(scope="session", autouse=True)
def compile_and_install():
    cmd = ["make", "python-develop"]
    sp.check_call(cmd)
