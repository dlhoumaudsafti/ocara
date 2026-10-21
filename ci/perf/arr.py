import sys
n=int(sys.argv[1]); xs=[]
for i in range(n): xs.append(i)
tot=0
for r in range(10):
    for i in range(n): tot+=xs[i]
print(tot)
