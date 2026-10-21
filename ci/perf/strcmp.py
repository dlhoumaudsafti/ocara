import sys
n=int(sys.argv[1]); words=["alpha","beta","gamma","delta"]; hits=0
for i in range(n):
    for w in words:
        if w=="gamma": hits+=1
print(hits)
