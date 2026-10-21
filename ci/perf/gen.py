import sys
def numbers(n):
    for i in range(n): yield i
n=int(sys.argv[1]); t=0
for v in numbers(n): t+=v
print(t)
