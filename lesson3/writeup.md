<div align="center">
<h1 style="text-align:center">LVN 🎉 | Ethan Uppal</h1>
</div>

> [!NOTE]
> This post has images and embedded interactive code previews which may take a few seconds to load.

## Source Code

- Source code for trivial dead code elimination passes: https://github.com/ethanuppal/cs6120/tree/main/lesson3/tdce
- Source code for local value numbering: https://github.com/ethanuppal/cs6120/tree/main/lesson3/lvn

I spent a few hours configuring CI for both of the above optimizations.
That serves as proof that my optimizations do work and that they work on **every
single benchmark file**. You can view the CI status here: https://github.com/ethanuppal/cs6120/actions/runs/13193206899/job/36829808979

![](https://github.com/ethanuppal/cs6120/blob/main/lesson3/ci_passing.png?raw=true)

The CI takes a bit to run because of some benchmarks which required me to set the `brench` timeout to 200 seconds.

As usual, all the passes are scripts that either take in Bril programs from standard input or a file and produce Bril's textual representation. I made some small tweaks to the `build_cfg` library/tool I had made for lesson 2. I've continued to use Conventional Commits style, so you can easily view the history of the repository. Most of the LVN work was done in a squash merge of ethanuppal/cs6120#4.

I wrote a wrapper script that you can pipe the output of `brench` into and receive:

1. Nice colored output showing the optimization status and performance of different pipelines (e.g., plain LVN is a nop on this benchmark, TDCE is faster on this benchmark).
2. Automatically errors if an optimization pass is slower

https://github.com/ethanuppal/cs6120/blob/46de2372c7b32109cf56b4e3ed16bcd1f54e03d9/lesson3/check_brench.py#L1-L49

Here's example output:

![Example output from my script](https://github.com/ethanuppal/cs6120/blob/main/lesson3/example_check_brench_output.png?raw=true)

## Trivial Dead Code Elimination

I implemented both dead code elimination passes shown in the video.

https://github.com/ethanuppal/cs6120/blob/46de2372c7b32109cf56b4e3ed16bcd1f54e03d9/lesson3/tdce/src/main.rs#L22-L48

https://github.com/ethanuppal/cs6120/blob/46de2372c7b32109cf56b4e3ed16bcd1f54e03d9/lesson3/tdce/src/main.rs#L50-L82

I tested them by running `brench` over every benchmark file and the example `test/tdce` files.

## Local Value Numbering

I implemented LVN with a basic value interner. It supports every single Bril instruction and program and works on every single benchmark.

https://github.com/ethanuppal/cs6120/blob/629e60992c8cc66c8b2762d38a768695e35985fc/lesson3/lvn/src/main.rs#L45-L52

Here's something I thought was funny. To make sure that `call`s or `allocs`s (which have anti-LVN semantics) don't mess things up, I created a variant which held a unit `struct` as follows:

https://github.com/ethanuppal/cs6120/blob/629e60992c8cc66c8b2762d38a768695e35985fc/lesson3/lvn/src/main.rs#L22-L31

I tested my implementation by running `brench` over every benchmark file and the example `test/tdce` files.

I had two issues when implementing LVN.

1. Floating-point literals are sometimes specified without decimal points. Since I represented an interned value with a sum type, I initially had a single "constant value" variant that conflated integers and floats. Adding a special variant just for floats fixed this issue.
2. Finally, I was failing only three tests due to weird pointer offset issues. After preventing subexpression recall of `alloc`, I was passing all benchmark files.

> [!NOTE]
> One thing to note is my strategy for coming up with new names is not entirely robust. I have a counter that strictly increments whenever a new temporary (that is, before the final assignment in a basic block) and appends `__t{counter}` to the end of the variable name, which could cause collisions, however unlikely. A better implementation (which is one I did in my builder API for the Calyx intermediate representation, a language for building hardware accelerators) is to prefix all existing identifiers to guarantee no name collisions when you introduce an identifier without that prefix.

## Extensions

### Commutativity 

https://github.com/ethanuppal/cs6120/blob/4caa5fa2de8b7935b5e6f46801b1820089f68adc/lesson3/lvn/src/main.rs#L247-L270

```diff
 $ bril2json <../bril/examples/test/lvn/commute.bril | ../target/debug/lvn | diff ../bril/examples/test/lvn/commute.bril -
1,2c1
< # (a + b) * (b + a)
< @main {
---
> @main() {
6,7c5,6
<   sum2: int = add b a;
<   prod: int = mul sum1 sum2;
---
>   sum2: int = id sum1;
>   prod: int = mul sum1 sum1;
```

### Constant Folding

https://github.com/ethanuppal/cs6120/blob/a62a19f2a3584df6e9a499e726225912e86a3b27/lesson3/lvn/src/main.rs#L306-L318

```diff
$ bril2json <simple_fold.bril | ../target/debug/lvn | diff simple_fold.bril -
1c1
< @main {
---
> @main() {
4c4
<   c: int = add a b;
---
>   c: int = const 3;
```

## Performance

Here's a quick comparison of how the optimization passes affect performance (I didn't include the LVN extensions). I wrote a simple script called `make_better_chart.py`:

https://github.com/ethanuppal/cs6120/blob/0c07b8b472b4ee661b316cabb4dee829e55ab1bb/lesson3/make_better_chart.py#L1-L13

To get the data yourself, you can run `brench brench.toml ../bril/benchmarks/**/*.bril ../bril/examples/test/tdce/*.bril | python3 make_better_chart.py > stats.csv` from the `lesson3` directory.

I graphed the data -- that seemed like a low-hanging fruit. The massive range of orders of magnitude meant that even after I split the data some small benchmarks times were not visible, and the low resolution meant not all benchmarks names appeared visibly on the x-axis. The important thing to notice is that the green bar is smaller and the yellow bar is usually smaller, so TDCE / LVN + TDCE is doing something. (You can make out the key at the top of the first chart).

![](https://github.com/ethanuppal/cs6120/blob/main/lesson3/chart1.png?raw=true)
![](https://github.com/ethanuppal/cs6120/blob/main/lesson3/chart2.png?raw=true)
