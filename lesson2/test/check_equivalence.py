#!/usr/bin/env python

import sys, os, subprocess, json, multiprocessing


def parse_args():
    package = sys.argv[1]
    executable = sys.argv[2]
    transformer = sys.argv[3]
    args = sys.argv[4:]
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
        package,
        executable,
        transformer,
        [
            filename
            for filename in filenames
            if not any(isinstance(e, str) and filename.endswith(e) for e in exclude)
        ],
    )


def init_worker(shared_event):
    global event
    event = shared_event


def check_file(args):
    (executable, transformer, filename) = args
    given_code = subprocess.check_output(
        f"bril2json <{filename}", shell=True, stderr=subprocess.DEVNULL
    ).decode("utf-8")
    passthrough_code = subprocess.check_output(
        f"{transformer} <{filename} | {executable} | bril2json",
        shell=True,
        stderr=subprocess.DEVNULL,
    ).decode("utf-8")
    given_bril = json.loads(given_code)
    passthrough_bril = json.loads(passthrough_code)

    if given_bril == passthrough_bril:
        print(f"{filename} OK")
    else:
        print(
            f"\x1b[31;1m{filename} ERROR\x1b[m\n\n--GIVEN--\n{json.dumps(given_bril)}\n\n--GOT--\n{json.dumps(passthrough_bril)}"
        )
        event.set()


if __name__ == "__main__":
    if not os.getcwd().endswith("lesson2"):
        print("Run from lesson2/")
        sys.exit(1)

    package, executable, transformer, filenames = parse_args()

    print(f"Rebuilding {package}")
    os.system(f"cargo build --package {package}")

    # https://superfastpython.com/multiprocessing-pool-stop-all-tasks-on-failure/
    with multiprocessing.Manager() as manager:
        shared_event = manager.Event()

        with multiprocessing.Pool(
            multiprocessing.cpu_count(),
            initializer=init_worker,
            initargs=(shared_event,),
        ) as pool:
            pool.imap_unordered(
                check_file,
                [(executable, transformer, filename) for filename in filenames],
            )
            pool.close()
            pool.join()
            if shared_event.is_set():
                print("Some tests failed!")
                pool.terminate()
                sys.exit(1)
