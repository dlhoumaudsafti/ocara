import sys
n=int(sys.argv[1]); ti=0; tf=0.0
for i in range(n):
    ti+=int(f"{i}")
    tf+=float(f"{i}.5")
print(ti, tf)
