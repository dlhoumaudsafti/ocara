import sys,asyncio
async def sq(n): return n*n
async def main(n):
    tot=0
    for i in range(n): tot+=await asyncio.create_task(sq(i))
    print(tot)
asyncio.run(main(int(sys.argv[1])))
