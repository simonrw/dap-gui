from IPython import start_ipython
import argparse

from pythondap.pythondap import Debugger, PausedFrame

class DebugSession:
    def __init__(self, breakpoints, file, config_path, config_name=None):
        self.d = Debugger(breakpoints=breakpoints, config_path=config_path, config_name=config_name, file=file)
        self.stack: list = []
        self.frame: PausedFrame | None = None

        self.resume = self.fn_resume
        self.step_over = self.fn_step_over

    def fn_resume(self):
        state = self.d.resume()
        print(f"received state: {state=}")
        if not state:
            return
        self.stack = state.stack
        self.frame = state.paused_frame
        return state

    def fn_step_over(self):
        state = self.d.step_over()
        print(f"received state: {state=}")
        if not state:
            return
        self.stack = state.stack
        self.frame = state.paused_frame
        return state

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("-b", "--breakpoint", type=int, nargs="*", default=[])
    parser.add_argument("-f", "--file", required=False)
    parser.add_argument("launch_configuration")
    parser.add_argument("-n", "--configuration")
    args = parser.parse_args()

    ns = DebugSession(breakpoints=args.breakpoint, file=args.file, config_path=args.launch_configuration, config_name=args.configuration)

    start_ipython(user_ns=ns.__dict__)
