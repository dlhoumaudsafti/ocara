import sys
n=int(sys.argv[1]); m={}
for i in range(n): m[f"k{i}"]=i
tot=0
for i in range(n): tot+=m[f"k{i}"]
print(tot)
