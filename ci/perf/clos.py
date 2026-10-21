import sys
def main():
    n=int(sys.argv[1]); count=0
    def add(k):
        nonlocal count
        count+=k
    for i in range(n): add(i%3)
    print(count)
main()
