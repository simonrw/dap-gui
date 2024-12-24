#!/usr/bin/env python

import os
from IPython import start_ipython
import subprocess as sp
from IPython.terminal.embed import InteractiveShellEmbed
import argparse

from pythondap import Debugger, PausedFrame

class DebugSession:
    def __init__(self, breakpoints, file):
        self.d = Debugger(breakpoints=breakpoints, file=file)
        self.stack: list = []
        self.frame: PausedFrame | None = None

        self.resume = self.fn_resume

    def fn_resume(self):
        state = self.d.resume()
        print(f"received state: {state=}")
        if not state:
            return
        self.stack = state.stack
        self.frame = state.paused_frame
        return state


try:
    parser = argparse.ArgumentParser()
    parser.add_argument("-b", "--breakpoint", type=int, nargs="*", default=[])
    parser.add_argument("-f", "--file", required=False)
    args = parser.parse_args()

    # start debuee in background process
    p = sp.Popen(["make", "run-attach"])

    ns = DebugSession(args.breakpoint, args.file)

    start_ipython(user_ns=ns.__dict__)
finally:
    p.kill()
