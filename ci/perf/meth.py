import sys
class Counter:
    def __init__(self): self.total=0
    def add(self,n): self.total+=self.weight(n)
    def weight(self,n): return n*2
n=int(sys.argv[1]); c=Counter()
for i in range(n): c.add(i)
print(c.total)
