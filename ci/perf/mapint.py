import sys
n=int(sys.argv[1]); m={}
for i in range(n): m[i]=i*2
t=0
for i in range(n): t+=m[i]
print(t)
