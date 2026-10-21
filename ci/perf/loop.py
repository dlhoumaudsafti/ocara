import sys
def fib(n): return n if n<2 else fib(n-1)+fib(n-2)
n=int(sys.argv[1]); acc=0.0
for i in range(n): acc+= (i%7)*0.5
print(fib(30), acc)
