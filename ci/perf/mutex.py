import sys, threading
n=int(sys.argv[1]); m=threading.Lock(); c=0
for i in range(n):
    m.acquire(); c+=1; m.release()
print(c)
