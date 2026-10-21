import sys
n=int(sys.argv[1]); tot=0
for i in range(n): tot+=len("hello world hello".replace("hello","bye").upper())
print(tot)
