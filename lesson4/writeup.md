- Code: https://github.com/ethanuppal/cs6120/tree/main/lesson4/dataflow
- CI passing: https://github.com/ethanuppal/cs6120/actions/runs/13322650730/job/37209946152

I first created a [simple `trait` extension](https://github.com/ethanuppal/cs6120/tree/main/lesson4/bril-util) for `bril_rs::Instruction` to give me
kill/gen/etc sets.

Then, I implemented a general solver for dataflow analysis in Rust, and
specifically implemented and tested:

- Reaching definitions
- Live variables

As usual, these are nicely
[`snafu`](https://docs.rs/snafu/latest/snafu/index.html)'d CLI apps.

I handchecked and [tested with `turnt`](https://github.com/ethanuppal/cs6120/tree/main/lesson4/dataflow/turnt) the examples in `examples/test/df` with both of the analyses.

For the dataflow solver, I looked at [some CMU slides](https://www.cs.cmu.edu/afs/cs/academic/class/15411-f20/www/rec/f20-03.pdf) which said to do postorder and reverse-postorder basic block orderings (generalizing the topological sort strategy Professor Sampson proposed in class).

To test reaching definitions, I manually checked that every definition the
analysis claimed to be reaching actually was by BFSing from each block + claimed
definition, running it on all the core benchmarks (see [check.py](https://github.com/ethanuppal/cs6120/blob/main/lesson4/dataflow/check.py)).
Notably, this is **not** _sufficient_ for a correct reaching definitions analysis,
only _necessary_. However, I have good reason for it to be correct based on the
simplicity of the code and the fact that I more rigorously confirmed the
correctness of live variable analysis (the two differ only in their transfer
function and traversal direction).

To test live variables, I matched my output with that of Professor Sampson's
`df.py`. I ran it only on all core benchmarks with a few excluded because my CFG
builder and Professor Sampson's differ on the definition of a basic block. Thus,
to avoid extensive manual pruning, I only used the core benchmarks instead of
all the benchmarks. Oh, I also ran it on the `examples/test/df/` programs.

For reference, here's the relevant CI code:

```yaml
  - name: Test reaching definitions analysis
    run: |
      cd lesson4/dataflow
      python3 check.py def ../../bril/benchmarks/**/*.bril ../../bril/examples/test/df/*.bril
  - name: Test live variables analysis
    run: |
      cd lesson4/dataflow
      python3 match_outputs.py \
        "python3 ../../bril/examples/df.py live | grep '  in:'" \
        "cargo run --package dataflow --quiet -- --analysis live | grep in:" \
        ../../bril/benchmarks/core/*.bril ../../bril/examples/test/df/*.bril \
        --exclude is-decreasing.bril --exclude recfact.bril --exclude relative-primes.bril # differ on definition of basic block
  - name: Snapshot test analyses on examples/test/df/ programs
    run: |
      cd lesson4/dataflow
      cd turnt && turnt df_copied_from_bril/*.bril
```

I believe I deserve a Michelin star because I implemented the assigned tasks and
tested my implementations.

## In other news

- I've been working on making `bril-frontend` a lossless parser so I can satiate
my perfectionism with a `brilfmt`. This won't be useful at all, but I am OCD
about it.
- Haven't pushed this work yet, but going to implement basic Hindley-Milner in
`bril-frontend` so the memory extension can be supported fully.
