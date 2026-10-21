import sys,json
n=int(sys.argv[1]); tot=0
for i in range(n):
    t=json.dumps({"id":i,"name":"ocara","ok":True,"tags":[1,2,3]}); b=json.loads(t); tot+=len(t)+len(b)
print(tot)
