import sys
class Point:
    def __init__(s,x,y): s.x=x; s.y=y
    def dist(s,o): return (s.x-o.x)*(s.x-o.x)+(s.y-o.y)*(s.y-o.y)
n=int(sys.argv[1]); pts=[Point(i,i*2) for i in range(n)]; tot=0
for i in range(1,n): tot+=pts[i].dist(pts[i-1])
print(tot)
