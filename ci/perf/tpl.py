import sys,time
n=int(sys.argv[1]); t=time.perf_counter(); tot=0; name="ocara"
for i in range(n):
    s=f"id={i} name={name} ok"; tot+=len(s)
print(tot)
