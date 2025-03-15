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


for i in range(1, len(rows), 4):
    baseline = rows[i]
    ssa = rows[i + 1]
    through_ssa = rows[i + 2]
    tdce_ssa = rows[i + 3]

    if (
        ssa[2] == "incorrect"
        or through_ssa[2] == "incorrect"
        or tdce_ssa == "incorrect"
    ):
        print(f"\x1b[31m{baseline[0]} INCORRECT\x1b[m")
        sys.exit(1)
    elif (
        ssa[2] == "incorrect"
        or through_ssa[2] == "incorrect"
        or tdce_ssa == "incorrect"
    ):
        print(f"\x1b[31m{baseline[0]} TIMED OUT\x1b[m")
        sys.exit(1)
    elif (
        ssa[2] == "incorrect"
        or through_ssa[2] == "incorrect"
        or tdce_ssa == "incorrect"
    ):
        print(f"\x1b[31m{baseline[0]} MISSING\x1b[m")
        sys.exit(1)

    baseline_time = int(baseline[2])
    ssa_time = int(ssa[2])
    through_ssa_time = int(through_ssa[2])
    tdce_ssa_time = int(tdce_ssa[2])

    print(f"{baseline[0]}")
    check_did_optimize(baseline_time, ssa_time, "into ssa")
    check_did_optimize(baseline_time, through_ssa_time, "into ssa | out of ssa")
    check_did_optimize(baseline_time, tdce_ssa_time, "into ssa | tdce")
    times_scored = sorted(
        [
            (baseline_time, "baseline"),
            (ssa_time, "into ssa"),
            (through_ssa_time, "into ssa | out of ssa"),
            (tdce_ssa_time, "into ssa | tdce"),
        ]
    )
    print(f"  (times in order: {times_scored})")
