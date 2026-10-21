import sys, time
n=int(sys.argv[1]); tot=0
for i in range(n): tot+=len(time.strftime("%d/%m/%Y %H:%M:%S", time.gmtime(1700000000+i*37)))
print(tot)
