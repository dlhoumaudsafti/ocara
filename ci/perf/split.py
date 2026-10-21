import sys
n=int(sys.argv[1]); line="alpha,beta,gamma,delta,epsilon,zeta"; tot=0
for i in range(n): tot+=len(line.split(","))
print(tot)
