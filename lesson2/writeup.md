- Benchmark: [ethanuppal/cs6120/lesson2/benchmark.bril](https://github.com/ethanuppal/cs6120/blob/main/lesson2/benchmark.bril). I don't have a PR because Bril doesn't support the instructions needed. I implemented and [opened a PR](https://github.com/sampsyo/bril/pull/352) for this.
- Tool:
i
**Benchmark**

- I implemented the Quake fast inverse square root algorithm and tested it on a few inputs (I imagine it could be easily fuzz-tested) after implementing bit casting primitives in Bril.

**CFG Builder**
- I wrote the CFG builder in Rust and tested it against every single Bril file in the `benchmarks` directory by ensuring that my CFG parser+printer was a roundtrip nop. This is tested on CI using a simple parallel Python runner I cooked up.

**bril-frontend**
- I wrote a custom 3000LOC frontend library for Bril's textual format in a day. The AST uses lifetimes for the text and all nodes are wrapped in a `Loc<T>` deref type. Extra care was put into good error messages and parser recovery. I also have a full pretty printer using my `inform` library for pretty-printing. I used expect testing on the lexer and parser (using `insta`; for both success and failures) as well as roundtripping on every single Bril file in the `core` benchmarks directly. This is also tested on CI.

**bril-lsp**
- I wrote a language server for Bril using Bril-frontend that supports autocompletion, hover, document symbols, and diagnostic reporting.
- I wrote Neovim and VSCode clients for the language server.

I think my works at least a Michelin star:
> indicating excellent implementation work, insightful participation, a thoughtful blog post, or a spectacularly successful project.

I believe these projects are successful and represent solid implementation work. I've spent over 20 hours working on them.
