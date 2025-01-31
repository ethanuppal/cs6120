macro_rules! type_snapshot {
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

            use std::fmt::Write;
            let mut context = std::collections::HashMap::new();
            let mut snapshot = String::new();
            for function in &program.functions {
                let (parameters, return_type, env) =
                    match bril_frontend::infer_types::type_infer_function(
                        &context, function,
                    ) {
                        Ok(result) => result,
                        Err(diagnostic) => {
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
                            panic!("Failed to type check program");
                        }
                    };

                // uses btreemap so ordering is consistent
                let _ = writeln!(&mut snapshot, "FUNCTION {}", function.name);
                for (variable, ty) in env {
                    let _ = writeln!(&mut snapshot, "  {}: {}", variable, ty);
                }

                context.insert(
                    function.name.to_string(),
                    (parameters, return_type),
                );
            }
            assert_snapshot!(format!(
                "PROGRAM\n--------\n{}\n\nTYPES\n-------\n{}",
                code, snapshot
            ));
        }
    };
}

//macro_rules! type_error {
//    ($name:ident, $code:expr) => {
//        #[test]
//        fn $name() {
//            use bril_frontend::loc::Loc;
//            use insta::assert_debug_snapshot;
//            use logos::Logos;
//
//            let code = $code;
//            let mut lexer = bril_frontend::lexer::Token::lexer(code);
//            let mut tokens = vec![];
//            while let Some(next) = lexer.next() {
//                if let Ok(token) = next {
//                    tokens.push(Loc::new(token, lexer.span()));
//                } else {
//                    panic!("Failed to lex. Leftover: {}", lexer.remainder());
//                }
//            }
//
//            let mut parser = bril_frontend::parser::Parser::new(&tokens);
//
//            let Err(()) = parser.parse_program() else {
//                panic!("Parsing invalid program should have produced an
// error");            };
//
//            assert_debug_snapshot!(parser.diagnostics());
//        }
//    };
//}

type_snapshot! {
    add_no_print_bril_checks,
    include_str!("../bril-programs/add_no_print.bril")
}

type_snapshot! {
    add_bril_checks,
    include_str!("../bril-programs/add.bril")
}

type_snapshot! {
    import_bril_checks,
    include_str!("../bril-programs/import.bril")
}

type_snapshot! {
    simple_bril_checks,
    include_str!("../bril-programs/simple.bril")
}

type_snapshot! {
    complex_bril_checks,
    r#"
@inc(a: int): int {
    one = const 1;
    b = add a one;
    ret b;
}

@main() {
    three = const 3;
    four = const 4;
    a = call @inc three;
    test = eq a four;
    print test;
}
    "#
}
