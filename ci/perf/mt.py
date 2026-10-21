import sys, threading
class Point:
    def __init__(self,x): self.x=x
n=int(sys.argv[1])
t=threading.Thread(target=lambda: None); t.start(); t.join()
pts=[]
for i in range(n): pts.append(Point(i))
tot=0
for p in pts: tot+=p.x
print(tot)
