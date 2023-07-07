def main():
    print("Hello world")

    with open("out.txt", "w") as outfile:
        outfile.write("ok")

if __name__ == "__main__":
    main()
