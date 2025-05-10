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
    insert_preheader = rows[i + 1]

    if insert_preheader[2] == "incorrect":
        print(f"\x1b[31m{baseline[0]} INCORRECT\x1b[m")
        sys.exit(1)
    elif insert_preheader[2] == "timeout":
        print(f"\x1b[31m{baseline[0]} TIMED OUT\x1b[m")
        sys.exit(1)
    elif insert_preheader[2] == "missing":
        print(f"\x1b[31m{baseline[0]} MISSING\x1b[m")
        sys.exit(1)

    baseline_time = int(baseline[2])
    loop_time = int(insert_preheader[2])

    print(f"{baseline[0]}")
    check_did_optimize(baseline_time, loop_time, "insert-preheader")
    times_scored = sorted(
        [
            (baseline_time, "baseline"),
            (loop_time, "insert-preheader"),
        ]
    )
    print(f"  (times in order: {times_scored})")
