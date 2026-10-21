import sys
class Weight:
    def weight(self,n): raise NotImplementedError
class Double(Weight):
    def weight(self,n): return n*2
class Triple(Weight):
    def weight(self,n): return n*3
def total(w,n):
    t=0
    for i in range(n): t+=w.weight(i)
    return t
n=int(sys.argv[1])
print(total(Double(),n)+total(Triple(),n))
