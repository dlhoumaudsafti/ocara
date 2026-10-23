import sys
class Pt:
    def __init__(self): self.x=0
    def dist(self,n): return abs(n-self.x)
class Acc:
    def __init__(self): self.total=0
    def add(self,p,n): self.total+=p.dist(n)
n=int(sys.argv[1]); p=Pt(); a=Acc()
for i in range(n): a.add(p,i)
print(a.total)
