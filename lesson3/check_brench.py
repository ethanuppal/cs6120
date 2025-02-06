import sys, csv

rows = list(csv.reader(sys.stdin))

for i in range(1, len(rows), 2):
    baseline = rows[i]
    tdce = rows[i + 1]
    if tdce[2] == "incorrect":
        print(f"\x1b[31m{baseline[0]} INCORRECT\x1b[m")
        sys.exit(1)
    elif tdce[2] > baseline[2]:
        print(f"\x1b[31m{baseline[0]} SLOWER\x1b[m")
        sys.exit(1)
    elif tdce[2] < baseline[2]:
        print(f"\x1b[32m{baseline[0]} FASTER\x1b[m")
    else:
        print(f"\x1b[33m{baseline[0]} NOP\x1b[m")
