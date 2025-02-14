import sys, subprocess, json
import multiprocessing
import difflib


def parse_args():
    if len(sys.argv) < 4:
        print(
            "usage: python3 match_outputs.py <oracle> <tested> <file>... [--exclude <file>]..."
        )
    oracle = sys.argv[1]
    tested = sys.argv[2]
    args = sys.argv[3:]
    filenames = []
    exclude = []
    all_are_args = False
    next_is_exclude = False
    for arg in args:
        if all_are_args:
            filenames.append(arg)
        elif arg == "--":
            all_are_args = True
        elif arg == "--exclude":
            next_is_exclude = True
        else:
            if next_is_exclude:
                exclude.append(arg)
                next_is_exclude = False
            else:
                filenames.append(arg)
    return (
        oracle,
        tested,
        [
            filename
            for filename in filenames
            if not any(isinstance(e, str) and filename.endswith(e) for e in exclude)
        ],
    )


def init_worker(shared_failure_event, shared_oracle, shared_tested):
    global failure_event
    global oracle
    global tested
    failure_event = shared_failure_event
    oracle = shared_oracle
    tested = shared_tested


def check_file(file):
    oracle_output = subprocess.getoutput(f"bril2json <{file} | {oracle}")
    my_output = subprocess.getoutput(f"bril2json <{file} | {tested}")
    if oracle_output == my_output:
        print(f"\x1b[32m{file} OK\x1b[m")
    else:
        print(f"\x1b[31m{file} ERROR\x1b[m")
        failure_event.set()

        red = lambda text: f"\033[38;2;255;0;0m{text}\033[m"
        green = lambda text: f"\033[38;2;0;255;0m{text}\033[m"
        blue = lambda text: f"\033[38;2;0;0;255m{text}\033[m"
        white = lambda text: f"\033[38;2;255;255;255m{text}\033[m"
        gray = lambda text: f"\033[38;2;128;128;128m{text}\033[m"

        diff = difflib.ndiff(oracle_output.splitlines(), my_output.splitlines())
        print("--- DIFF ---")
        for line in diff:
            if line.startswith("+"):
                print(green(line))
            elif line.startswith("-"):
                print(red(line))
            elif line.startswith("^"):
                print(blue(line))
            elif line.startswith("?"):
                print(gray(line))
            else:
                print(white(line))


if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("usage: python3 test.py <oracle> <tested> <file>...")
        sys.exit(1)
    (oracle, tested, files) = parse_args()

    with multiprocessing.Manager() as manager:
        failure_event = manager.Event()

        with multiprocessing.Pool(
            multiprocessing.cpu_count(),
            initializer=init_worker,
            initargs=(failure_event, oracle, tested),
        ) as pool:
            pool.imap_unordered(check_file, files)
            pool.close()
            pool.join()
            if failure_event.is_set():
                print("Exiting due to errors")
                pool.terminate()
                sys.exit(1)
