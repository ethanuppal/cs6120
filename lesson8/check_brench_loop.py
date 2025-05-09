import sys, csv

rows = list(csv.reader(sys.stdin))

allow_slower = len(sys.argv) >= 2 and sys.argv[1] == "--allow-slower"


def check_did_optimize(baseline, new, name):
    global allow_slower

    if new > baseline:
        print(f"> \x1b[31m{name} SLOWER ({name}: {new}, baseline: {baseline})\x1b[m")
        if not allow_slower:
            sys.exit(1)
    elif new < baseline:
        print(f"> \x1b[32m{name} FASTER ({name}: {new}, baseline: {baseline})\x1b[m")
    else:
        print(f"> \x1b[33m{name} NOP ({name}: {new}, baseline: {baseline})\x1b[m")


for i in range(1, len(rows), 2):
    baseline = rows[i]
    loop = rows[i + 1]

    if loop[2] == "incorrect":
        print(f"\x1b[31m{baseline[0]} INCORRECT\x1b[m")
        sys.exit(1)
    elif loop[2] == "missing":
        print(f"\x1b[31m{baseline[0]} MISSING\x1b[m")
        sys.exit(1)

    baseline_time = int(baseline[2])
    loop_time = int(loop[2])

    print(f"{baseline[0]}")
    check_did_optimize(baseline_time, loop_time, "loop-opt")
    times_scored = sorted(
        [
            (baseline_time, "baseline"),
            (loop_time, "loop-opt"),
        ]
    )
    print(f"  (times in order: {times_scored})")
