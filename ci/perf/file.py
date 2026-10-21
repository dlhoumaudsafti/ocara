import sys, os
# Dans une fonction : `text +=` ne reste linéaire que sur une variable locale.
def main():
    n=int(sys.argv[1]); path=f"/tmp/ocara_perf_file_py_{os.getpid()}.txt"
    text=""
    for i in range(n): text+=f"ligne {i}\n"
    with open(path,"w") as f: f.write(text)
    with open(path) as f: lines=f.read().split("\n")
    tot=sum(len(l) for l in lines)
    os.remove(path)
    print(len(lines), tot)
main()
