import sys
n=int(sys.argv[1]); xs=list(range(1000)); tot=0
for i in range(n):
    if (i%1500) in xs: tot+=1
    tot+=xs.index(i%1000)
    part=xs[i%900:i%900+50]
    tot+=list(reversed(part))[0]
    xs.append(i)
    tot+=xs.pop()
print(tot)
