a = 10

def foo():
    return 5

class Bar:
    def __init__(self, value):
        self.value = value

def main():
    b = 20
    c = Bar(10)
    foo()
    print("Hello world")

    with open("out.txt", "w") as outfile:
        outfile.write("ok")

if __name__ == "__main__":
    main()
