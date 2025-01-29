#!/usr/bin/env python

import sys, os, subprocess, json, multiprocessing


def parse_args():
    args = sys.argv[1:]
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
    return [filename for filename in filenames if filename not in exclude]


def init_worker(shared_event):
    global event
    event = shared_event


def check_file(filename):
    given_code = subprocess.check_output(
        f"bril2json <{filename}", shell=True, stderr=subprocess.DEVNULL
    ).decode("utf-8")
    passthrough_code = subprocess.check_output(
        f"bril2json <{filename} | ../target/debug/build-cfg --mode passthrough | bril2json",
        shell=True,
        stderr=subprocess.DEVNULL,
    ).decode("utf-8")
    given_bril = json.loads(given_code)
    passthrough_bril = json.loads(passthrough_code)

    if given_bril == passthrough_bril:
        print(f"{filename} OK")
    else:
        print(f"\x1b[31;1m{filename} ERROR\x1b[m")
        event.set()


if __name__ == "__main__":
    if not os.getcwd().endswith("lesson2"):
        print("Run from lesson2/")
        sys.exit(1)

    print("Rebuilding build-cfg")
    os.system("cargo build --package build-cfg")

    filenames = parse_args()

    # https://superfastpython.com/multiprocessing-pool-stop-all-tasks-on-failure/
    with multiprocessing.Manager() as manager:
        shared_event = manager.Event()

        with multiprocessing.Pool(
            multiprocessing.cpu_count(),
            initializer=init_worker,
            initargs=(shared_event,),
        ) as pool:
            try:
                pool.imap_unordered(check_file, filenames)
                pool.close()
                pool.join()
                if shared_event.is_set():
                    print("Some tests failed!")
                    sys.exit(1)
            except:
                pass
