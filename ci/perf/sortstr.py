import sys
n=int(sys.argv[1]); xs=[f"k{(i*7919)%100003}" for i in range(n)]; s=sorted(xs); print(s[0], s[-1])
