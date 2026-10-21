import sys
n=int(sys.argv[1]); xs=["a","b","c","d","e"]; tot=0
for i in range(n): tot+=len("-".join(xs))+len(str(i))
print(tot)
