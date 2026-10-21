import sys, re
n=int(sys.argv[1]); hits=0; out=0
for i in range(n):
    line=f"user{i}@example.com id={i}"
    if re.search(r"^[a-z]+[0-9]+@example\.com", line): hits+=1
    out+=len(re.sub(r"[0-9]+", "#", line, count=1))
print(hits, out)
