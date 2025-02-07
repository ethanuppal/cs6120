import sys, csv

rows = list(csv.reader(sys.stdin))

print("benchmark,baseline,tdce,lvn,lvn_tdce")

for i in range(1, len(rows), 4):
    name = rows[i][0]
    baseline = rows[i][2]
    tdce = rows[i + 1][2]
    lvn = rows[i + 2][2]
    lvn_tdce = rows[i + 3][2]
    print(f"{name},{baseline},{tdce},{lvn},{lvn_tdce}")
