#!/usr/bin/env python

import os
from IPython.terminal.embed import InteractiveShellEmbed
import argparse
# os.environ["RUST_LOG"] = "error"

from pythondap import Debugger, PausedFrame


parser = argparse.ArgumentParser()
parser.add_argument("-b", "--breakpoint", type=int, nargs="*", default=[])
parser.add_argument("-f", "--file", required=False)
args = parser.parse_args()



class PythonDebugger:
    def __init__(self):
        self.d = Debugger(breakpoints=args.breakpoint, file=args.file)
        self.stack: list = []
        self.frame: PausedFrame | None = None


    def resume(self):
        state = self.d.resume()
        self.stack = state.stack
        self.frame = state.paused_frame


# breakpoints = d.breakpoints

d = PythonDebugger()

resume = d.resume
def stack():
    return d.stack

def frame():
    return d.frame


ipshell = InteractiveShellEmbed(banner1 = '', exit_msg = '')
ipshell.run_line_magic("autocall", "2")
ipshell()
