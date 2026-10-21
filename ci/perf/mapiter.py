import sys
n=int(sys.argv[1]); m={f"k{i}":i for i in range(1000)}; tot=0
for r in range(n):
    for k,v in m.items(): tot+=v
print(tot)
