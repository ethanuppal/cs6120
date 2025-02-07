import sys, csv

rows = list(csv.reader(sys.stdin))


def check_did_optimize(baseline, new, name):
    if new > baseline:
        print(f"> \x1b[31m{name} SLOWER ({name}: {new}, baseline: {baseline})\x1b[m")
        sys.exit(1)
    elif new < baseline:
        print(f"> \x1b[32m{name} FASTER ({name}: {new}, baseline: {baseline})\x1b[m")
    else:
        print(f"> \x1b[33m{name} NOP ({name}: {new}, baseline: {baseline})\x1b[m")


for i in range(1, len(rows), 4):
    baseline = rows[i]
    tdce = rows[i + 1]
    lvn = rows[i + 2]
    lvn_tdce = rows[i + 3]

    if tdce[2] == "incorrect" or lvn[2] == "incorrect":
        print(f"\x1b[31m{baseline[0]} INCORRECT\x1b[m")
        sys.exit(1)
    elif tdce[2] == "timeout" or lvn[2] == "timeout":
        print(f"\x1b[31m{baseline[0]} TIMED OUT\x1b[m")
        sys.exit(1)
    elif tdce[2] == "missing" or lvn[2] == "missing":
        print(f"\x1b[31m{baseline[0]} MISSING\x1b[m")
        sys.exit(1)

    baseline_time = int(baseline[2])
    tdce_time = int(tdce[2])
    lvn_time = int(lvn[2])
    lvn_tdce_time = int(lvn_tdce[2])

    print(f"{baseline[0]}")
    check_did_optimize(baseline_time, tdce_time, "tdce")
    check_did_optimize(baseline_time, lvn_time, "lvn")
    check_did_optimize(baseline_time, lvn_tdce_time, "lvn | tdce")
    times_scored = sorted(
        [
            (baseline_time, "baseline"),
            (tdce_time, "tdce"),
            (lvn_time, "lvn"),
            (lvn_tdce_time, "lvn | tdce"),
        ]
    )
    print(f"  (times in order: {times_scored})")
