import argparse

import debugpy


a = 10

def foo():
    return 5

class Bar:
    def __init__(self, value):
        self.value = value


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("-p", "--port", type=int, required=False, default=5678)
    args = parser.parse_args()

    debugpy.listen(("127.0.0.1", args.port))
    debugpy.wait_for_client()

    b = 20
    c = Bar(10)
    foo()
    print("Hello world")

    with open("out.txt", "w") as outfile:
        outfile.write("ok")

if __name__ == "__main__":
    main()
