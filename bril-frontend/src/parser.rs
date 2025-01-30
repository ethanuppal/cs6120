// Copyright (C) 2024 Ethan Uppal.
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License only.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// this program.  If not, see <https://www.gnu.org/licenses/>.

use crate::{
    ast,
    lexer::Token,
    loc::{Loc, Span, Spanned, WithLocation},
};

pub struct Diagnostic {
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, spanned: impl Spanned) -> Self {
        Self {
            message: message.into(),
            span: spanned.span(),
        }
    }
}

pub struct Parser<'tokens, 'source: 'tokens> {
    index: usize,
    tokens: &'tokens [Loc<Token<'source>>],
    diagnostics: Vec<Diagnostic>,
}

pub type Result<T> = std::result::Result<T, ()>;

impl<'tokens, 'source: 'tokens> Parser<'tokens, 'source> {
    pub fn new(tokens: &'tokens [Loc<Token<'source>>]) -> Self {
        Self {
            index: 0,
            tokens,
            diagnostics: vec![],
        }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn is_eof(&self) -> bool {
        self.index == self.tokens.len()
    }

    pub fn eof_span(&self) -> Span {
        self.tokens
            .last()
            .map(|last| last.span.end..last.span.end)
            .unwrap_or(0..0)
    }

    pub fn get(&self, offset: usize) -> Option<&Loc<Token<'source>>> {
        if self.index + offset < self.tokens.len() {
            Some(&self.tokens[self.index + offset])
        } else {
            None
        }
    }

    pub fn is_at(&self, pattern: &Token) -> bool {
        self.get(0)
            .filter(|token| token.matches_against(pattern.clone()))
            .is_some()
    }

    pub fn advance(&mut self) {
        self.index += 1;
    }

    pub fn try_eat(&mut self, pattern: Token) -> Option<Loc<Token<'source>>> {
        if self.is_at(&pattern) {
            let result = self.get(0).unwrap().clone();
            self.advance();
            Some(result)
        } else {
            None
        }
    }

    pub fn eat(
        &mut self,
        pattern: Token,
        message: impl Into<String>,
    ) -> Result<Loc<Token<'source>>> {
        if self.is_at(&pattern) {
            let result = self.get(0).unwrap().clone();
            self.advance();
            Ok(result)
        } else {
            if let Some(current) = self.tokens.get(self.index) {
                self.diagnostics.push(Diagnostic::new(
                    format!(
                        "Unexpected '{}', expected '{}': {}",
                        current.pattern_name(),
                        pattern.pattern_name(),
                        message.into(),
                    ),
                    current,
                ));
            } else {
                self.diagnostics.push(Diagnostic::new(
                    format!(
                        "Unexpected EOF, expected '{}': {}",
                        pattern.pattern_name(),
                        message.into()
                    ),
                    self.eof_span(),
                ));
            }

            Err(())
        }
    }

    pub fn recover(
        &mut self,
        goals: impl IntoIterator<Item = Token<'source>>,
        message: impl Into<String>,
    ) {
        let mut goals = goals.into_iter();
        while !self.is_eof() {
            if goals.any(|goal| self.is_at(&goal)) {
                return;
            }
            self.index += 1;
        }

        self.diagnostics
            .push(Diagnostic::new(message, self.eof_span()));
    }

    pub fn parse_separated<'a, U, F: FnMut(&mut Self) -> Result<U>>(
        &mut self,
        mut f: F,
        separators: impl IntoIterator<Item = Token<'a>>,
        terminators: impl IntoIterator<Item = Token<'a>>,
        message: impl Into<String>,
    ) -> Result<(Vec<U>, Loc<Token<'source>>)> {
        let separators = separators.into_iter().collect::<Vec<_>>();
        let terminators = terminators.into_iter().collect::<Vec<_>>();

        let mut result = vec![];

        while !self.is_eof() {
            for terminator in &terminators {
                if let Some(terminator) = self.try_eat(terminator.clone()) {
                    return Ok((result, terminator));
                }
            }

            if self.is_eof() {
                break;
            }

            if !result.is_empty() {
                if separators
                    .iter()
                    .cloned()
                    .any(|separator| self.is_at(&separator))
                {
                    self.advance();
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        format!(
                        "Unexpected EOF, expected separator (one of: {}): {}",
                        separators
                            .iter()
                            .map(|separator| format!(
                                "'{}'",
                                separator.pattern_name()
                            ))
                            .collect::<Vec<_>>()
                            .join(", "),
                        message.into()
                    ),
                        self.eof_span(),
                    ));
                    return Err(());
                }
            }

            result.push(f(self)?);
        }

        self.diagnostics.push(Diagnostic::new(
            format!(
                "Unexpected EOF, expected terminator (one of: {}): {}",
                terminators
                    .into_iter()
                    .map(|terminator| format!(
                        "'{}'",
                        terminator.pattern_name()
                    ))
                    .collect::<Vec<_>>()
                    .join(", "),
                message.into()
            ),
            self.eof_span(),
        ));
        Err(())
    }

    pub fn parse_imported_function_alias(
        &mut self,
        as_token: Loc<()>,
    ) -> Result<Loc<ast::ImportedFunctionAlias<'source>>> {
        let name = self
            .eat(
                Token::FunctionName(""),
                "Expected function alias name after `as`",
            )?
            .map(Token::assume_function_name);

        Ok(ast::ImportedFunctionAlias {
            as_token: as_token.clone(),
            name: name.clone(),
        }
        .between(as_token, name))
    }

    pub fn parse_imported_function(
        &mut self,
    ) -> Result<Loc<ast::ImportedFunction<'source>>> {
        let name = self
            .eat(Token::FunctionName(""), "Expected function name to import")?
            .map(Token::assume_function_name);

        let mut alias = None;
        if let Some(as_token) = self.try_eat(Token::As) {
            alias = Some(
                self.parse_imported_function_alias(as_token.without_inner())?,
            );
        }

        let start = name.span();
        let end = alias
            .as_ref()
            .map(|alias| alias.span())
            .unwrap_or(name.span());
        Ok(ast::ImportedFunction { name, alias }.between(start, end))
    }

    pub fn parse_import(
        &mut self,
        from_token: Loc<()>,
    ) -> Result<Loc<ast::Import<'source>>> {
        let path = self
            .eat(Token::Path(""), "Expected path after `from`")?
            .map(Token::assume_path);

        let import_token = self
            .eat(Token::Import, "Expected `import` token after path")?
            .without_inner();

        let (imported_functions, terminator) = self.parse_separated(
            Self::parse_imported_function,
            [Token::Comma],
            [Token::Semi],
            "Failed to parse imported function",
        )?;

        Ok(ast::Import {
            from_token: from_token.clone(),
            path,
            import_token,
            imported_functions,
        }
        .between(&from_token, &terminator))
    }

    pub fn parse_label(&mut self) -> Result<Loc<ast::Label<'source>>> {
        let name = self
            .eat(Token::Label(""), "Expected label")?
            .map(Token::assume_label);
        let span = name.span();
        Ok(ast::Label { name }.at(span))
    }

    pub fn parse_instruction(
        &mut self,
    ) -> Result<Loc<ast::Instruction<'source>>> {
        todo!()
    }

    pub fn parse_function_code(
        &mut self,
    ) -> Result<Loc<ast::FunctionCode<'source>>> {
        if self.is_at(&Token::Label("")) {
            let label = self.parse_label()?;
            let colon_token = self
                .eat(Token::Colon, "Expected colon after label in function")?
                .without_inner();
            let start = label.span();
            let end = colon_token.span();
            Ok(ast::FunctionCode::Label { label, colon_token }
                .between(start, end))
        } else {
            let instruction = self.parse_instruction()?;
            let span = instruction.span();
            Ok(ast::FunctionCode::Instruction(instruction).at(span))
        }
    }

    pub fn parse_type(&mut self) -> Result<Loc<ast::Type>> {
        let ty = self
            .eat(Token::Identifier(""), "Expected type")?
            .map(Token::assume_identifier);

        Ok(match ty.inner {
            "int" => ast::Type::Int.at(ty),
            "float" => ast::Type::Float.at(ty),
            "char" => ast::Type::Char.at(ty),
            "ptr" => {
                self.eat(
                    Token::LeftAngle,
                    "Missing inner type for pointer type",
                )?;
                let inner = self.parse_type()?;
                let end = self.eat(
                    Token::RightAngle,
                    "Missing right angle after pointer inner type",
                )?;
                ast::Type::Ptr(Box::new(inner)).between(ty, end)
            }
            _ => {
                self.diagnostics.push(Diagnostic::new("Unknown type", ty));
                return Err(());
            }
        })
    }

    pub fn parse_type_annotation(
        &mut self,
    ) -> Result<Loc<ast::TypeAnnotation>> {
        let colon_token = self
            .eat(
                Token::Colon,
                format!("Need colon before type in type annotation"),
            )?
            .without_inner();
        let ty = self.parse_type()?;
        let start = colon_token.span();
        let end = ty.span();
        Ok(ast::TypeAnnotation { colon_token, ty }.between(start, end))
    }

    pub fn parse_function_parameter(
        &mut self,
    ) -> Result<(Loc<&'source str>, Loc<ast::TypeAnnotation>)> {
        let name = self
            .eat(Token::Identifier(""), "Expected parameter name")?
            .map(Token::assume_identifier);
        let annotation = self.parse_type_annotation()?;
        Ok((name, annotation))
    }

    pub fn parse_function(
        &mut self,
        name: Loc<&'source str>,
    ) -> Result<Loc<ast::Function<'source>>> {
        let parameters = if self.try_eat(Token::LeftPar).is_some() {
            self.parse_separated(
                Self::parse_function_parameter,
                [Token::Comma],
                [Token::RightPar],
                "Failed to parse function parameters",
            )?
            .0
        } else {
            vec![]
        };

        let return_type = if self.is_at(&Token::Colon) {
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        self.eat(Token::LeftBrace, "Missing left brace to open function")?;

        let mut body = vec![];
        while !self.is_eof() && !self.is_at(&Token::RightBrace) {
            body.push(self.parse_function_code()?);
        }

        let end =
            self.eat(Token::RightBrace, "Missing left brace to open function")?;

        Ok(ast::Function {
            name: name.clone(),
            parameters,
            return_type,
            body,
        }
        .between(name, end))
    }

    pub fn parse_program(&mut self) -> Result<ast::Program<'source>> {
        let mut imports = vec![];
        let mut functions = vec![];
        while !self.is_eof() {
            if let Some(from_token) = self.try_eat(Token::From) {
                if let Ok(import) =
                    self.parse_import(from_token.without_inner())
                {
                    imports.push(import);
                } else {
                    self.recover(
                        [Token::Import, Token::FunctionName("")],
                        "Failed to find another valid import or function to recover from",
                    );
                }
            } else if let Some(function_name) =
                self.try_eat(Token::FunctionName(""))
            {
                if let Ok(function) = self.parse_function(
                    function_name.map(Token::assume_function_name),
                ) {
                    functions.push(function);
                } else {
                    self.recover(
                        [Token::Import, Token::FunctionName("")],
                        "Failed to find another valid import or function to recover from",
                    );
                }
            } else {
                self.recover([Token::Import, Token::FunctionName("")],
                        "Failed to find another valid import or function to recover from",
                );
            }
        }

        if self.diagnostics.is_empty() {
            Ok(ast::Program { imports, functions })
        } else {
            Err(())
        }
    }
}
