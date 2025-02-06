import sys, csv

rows = list(csv.reader(sys.stdin))

for i in range(1, len(rows), 2):
    baseline = rows[i]
    tdce = rows[i + 1]

    if tdce[2] == "incorrect":
        print(f"\x1b[31m{baseline[0]} INCORRECT\x1b[m")
        sys.exit(1)

    print(baseline[0])
    baseline_time = int(baseline[2])
    tdce_time = int(tdce[2])

    if tdce_time > baseline_time:
        print(
            f"\x1b[31m{baseline[0]} SLOWER (tdce {tdce[2]} instructions, baseline {baseline[2]} instructions)\x1b[m"
        )
        sys.exit(1)
    elif tdce_time < baseline_time:
        print(f"\x1b[32m{baseline[0]} FASTER\x1b[m")
    else:
        print(f"\x1b[33m{baseline[0]} NOP\x1b[m")
