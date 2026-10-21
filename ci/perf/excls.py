import sys
class AppError(Exception):
    def __init__(self,message,code):
        super().__init__(message); self.code=code
n=int(sys.argv[1]); c=0
for i in range(n):
    try:
        raise AppError("x",i)
    except AppError:
        c+=1
print(c)
