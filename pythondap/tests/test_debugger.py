from pythondap import Debugger


def test_debugger_lifecycle(port):
    print("Creating debugger")
    d = Debugger.new_on_port(
        port=port,
        file="./test.py",
        breakpoints=[4],
    )
    print("Resuming execution")
    d.resume()
    # hit breakpoint
    d.resume()
    # program ended
