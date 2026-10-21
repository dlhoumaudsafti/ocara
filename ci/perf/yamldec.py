import sys, yaml
n=int(sys.argv[1])
text="".join(f"k{i}:\n  name: item{i}\n  value: {i}\n" for i in range(n))
loader=getattr(yaml, "CSafeLoader", yaml.SafeLoader)
doc=yaml.load(text, Loader=loader)
print(len(doc))
