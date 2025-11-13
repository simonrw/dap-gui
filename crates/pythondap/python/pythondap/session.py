from pythondap.pythondap import Debugger, PausedFrame


class DebugSession:
    def __init__(
        self,
        breakpoints,
        file,
        config_path,
        config_name=None,
        program=None,
        port: int | None = None,
    ):
        if port:
            print(f"Connecting on different {port=}")
            self.d = Debugger.new_on_port(
                port=port,
                breakpoints=breakpoints,
                config_path=config_path,
                config_name=config_name,
                file=file,
                program=program,
            )
        else:
            self.d = Debugger(
                breakpoints=breakpoints,
                config_path=config_path,
                config_name=config_name,
                file=file,
                program=program,
            )
        print("Created debugger")
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
