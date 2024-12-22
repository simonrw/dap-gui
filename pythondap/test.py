#!/usr/bin/env python

import os
from IPython.terminal.embed import InteractiveShellEmbed
import argparse
# os.environ["RUST_LOG"] = "error"

from pythondap import Debugger


parser = argparse.ArgumentParser()
parser.add_argument("-b", "--breakpoint", type=int, nargs="*", default=[])
parser.add_argument("-f", "--file", required=False)
args = parser.parse_args()

print(args)

d = Debugger(breakpoints=args.breakpoint, file=args.file)

# setup global functions
resume = d.resume
breakpoints = d.breakpoints


ipshell = InteractiveShellEmbed(banner1 = '', exit_msg = '')
ipshell.run_line_magic("autocall", "2")
ipshell()
