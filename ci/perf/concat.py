import sys
def main(n):
    s = ""
    for i in range(n):
        s = s + "x"
    print(len(s))
main(int(sys.argv[1]))
