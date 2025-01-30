macro_rules! parser_snapshot {
    ($name:ident, $code:expr) => {
        #[test]
        fn $name() {
            use bril_frontend::loc::Loc;
            use insta::assert_snapshot;
            use logos::Logos;

            let code = $code;
            let mut lexer = bril_frontend::lexer::Token::lexer(code);
            let mut tokens = vec![];
            while let Some(next) = lexer.next() {
                if let Ok(token) = next {
                    tokens.push(Loc::new(token, lexer.span()));
                } else {
                    panic!("Failed to lex. Leftover: {}", lexer.remainder());
                }
            }

            let mut parser = bril_frontend::parser::Parser::new(&tokens);

            let Ok(program) = parser.parse_program() else {
                for diagnostic in parser.diagnostics() {
                    println!("{}:", diagnostic.message);
                    for (text, span) in &diagnostic.labels {
                        println!("Label: {}", text);
                        println!(
                            "Code: `{}`",
                            &code[span
                                .clone()
                                .unwrap_or(diagnostic.span.clone())]
                        );
                    }
                }
                panic!("Failed to parse program");
            };

            let mut buffer = String::new();
            bril_frontend::printer::Printer::new(&mut buffer, 2)
                .print_program(&program)
                .expect("Failed to print program");

            assert_snapshot!(format!(
                "ORIGINAL\n--------\n{}\n\nPRINTED\n-------\n{}",
                code, buffer
            ));
        }
    };
}

parser_snapshot! {
    add_no_print_bril_parses,
    include_str!("../bril-programs/add_no_print.bril")
}

parser_snapshot! {
    add_bril_parses,
    include_str!("../bril-programs/add.bril")
}

parser_snapshot! {
    import_bril_parses,
    include_str!("../bril-programs/import.bril")
}

parser_snapshot! {
    simple_bril_parses,
    include_str!("../bril-programs/simple.bril")
}
