a = 10

def foo():
    return 5

def main():
    b = 20
    foo()
    print("Hello world")

    with open("out.txt", "w") as outfile:
        outfile.write("ok")

if __name__ == "__main__":
    main()
