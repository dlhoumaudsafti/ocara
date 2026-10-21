import sys
n=int(sys.argv[1]); c=0
for i in range(n):
    try:
        raise Exception("x")
    except Exception:
        c+=1
print(c)
