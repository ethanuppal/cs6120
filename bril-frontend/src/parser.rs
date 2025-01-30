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

    pub fn current(&self) -> &Loc<Token<'source>> {
        self.get(0)
            .expect("Unexpected EOF when accessing current token")
    }

    pub fn get(&self, offset: usize) -> Option<&Loc<Token<'source>>> {
        if self.index + offset < self.tokens.len() {
            Some(&self.tokens[self.index + offset])
        } else {
            None
        }
    }

    pub fn advance(&mut self) {
        self.index += 1;
    }

    pub fn try_eat(&mut self, pattern: Token) -> Option<Loc<Token<'source>>> {
        if self
            .get(0)
            .filter(|token| token.matches_against(pattern))
            .is_some()
        {
            let result = self.current().clone();
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
        if self
            .get(0)
            .filter(|token| token.matches_against(pattern.clone()))
            .is_some()
        {
            let result = self.current().clone();
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
            if goals.any(|goal| self.current().matches_against(goal)) {
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
                    .any(|separator| self.current().matches_against(separator))
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

        let start = name.span.clone();
        let end = alias
            .as_ref()
            .map(|alias| alias.span.clone())
            .unwrap_or(name.span.clone());
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
