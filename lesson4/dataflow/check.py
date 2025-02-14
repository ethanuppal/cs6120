import sys, subprocess
import multiprocessing


def init_worker(shared_failure_event):
    global failure_event
    failure_event = shared_failure_event


def check_file(args):
    (file, analysis) = args
    print(f"\x1b[33m{file} START\x1b[m")
    result = subprocess.run(
        f"bril2json <{file} | cargo run --quiet -- --analysis {analysis}",
        shell=True,
        capture_output=True,
    )
    if result.returncode == 0:
        print(f"\x1b[32m{file} OK\x1b[m")
    else:
        print(f"\x1b[31m{file} ERROR\x1b[m")
        print("---")
        print(result.stderr.decode("utf-8"))
        print("---")
        failure_event.set()


if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("usage: python3 check.py <analysis> <file>...")
        sys.exit(1)
    analysis = sys.argv[1]
    files = sys.argv[2:]

    with multiprocessing.Manager() as manager:
        failure_event = manager.Event()

        with multiprocessing.Pool(
            multiprocessing.cpu_count(),
            initializer=init_worker,
            initargs=(failure_event,),
        ) as pool:
            pool.imap_unordered(check_file, [(file, analysis) for file in files])
            pool.close()
            pool.join()
            if failure_event.is_set():
                print("Exiting due to errors")
                pool.terminate()
                sys.exit(1)
