PYTHON_CODE = """a = 10
b = 20
a = a + b
print(a)
"""


def test_execute(environment):
    with environment(PYTHON_CODE, breakpoints=[0]) as env:
        env.resume()
