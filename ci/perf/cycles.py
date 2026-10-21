import sys
class Node:
    def __init__(self): self.next=None; self.v=0
    def get(self): return self.v
n=int(sys.argv[1]); nodes=[]; prev=None
for i in range(n):
    x=Node(); x.v=i; x.next=prev; nodes.append(x); prev=x
tot=0
for r in range(5):
    for x in nodes: tot+=x.get()
print(tot)
