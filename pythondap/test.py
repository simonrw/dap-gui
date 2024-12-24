#!/usr/bin/env python

import os
from IPython import start_ipython
import subprocess as sp
from IPython.terminal.embed import InteractiveShellEmbed
import argparse
# os.environ["RUST_LOG"] = "error"

from pythondap import Debugger, PausedFrame

class InteractiveShellEmbedEnhanced(InteractiveShellEmbed):
    def __init__(self, breakpoints, file, *args, **kwargs):
        banner1 = kwargs.pop("banner1", "")
        exit_msg = kwargs.pop("exit_msg", "")

        super().__init__(banner1=banner1, exit_msg=exit_msg, *args, **kwargs)

        self.d = Debugger(breakpoints=breakpoints, file=file)
        self.stack: list = []
        self.frame: PausedFrame | None = None

    def resume(self):
        state = self.d.resume()
        self.stack = state.stack
        self.frame = state.paused_frame
        return state




parser = argparse.ArgumentParser()
parser.add_argument("-b", "--breakpoint", type=int, nargs="*", default=[])
parser.add_argument("-f", "--file", required=False)
args = parser.parse_args()


# start debuee in background process
p = sp.Popen(["make", "run-attach"])


ipshell = InteractiveShellEmbedEnhanced(breakpoints=args.breakpoint, file=args.file)

def resume():
    return ipshell.resume()

def stack():
    return ipshell.stack

def frame():
    return ipshell.frame

ipshell.run_line_magic("autocall", "2")
setattr(ipshell.__class__, 'user_global_ns', property(lambda self: self.user_ns))
ipshell()
