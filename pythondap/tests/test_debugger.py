from pythondap import Debugger


def test_debugger_lifecycle():
    d = Debugger(
        file="./test.py",
        breakpoints=[4],
    )
    d.resume()
    d.resume()