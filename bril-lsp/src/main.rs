// Copyright (C) 2024 Ethan Uppal.
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License only.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program.  If not, see <https://www.gnu.org/licenses/>.

use std::{fs, path::PathBuf, sync::Arc};

use bril_rs::Program;
use brilirs::{basic_block::BBProgram, check};
use dashmap::DashMap;
use tower_lsp::{
    jsonrpc, lsp_types::*, Client, LanguageServer, LspService, Server,
};

struct BuiltinCompletionItem {
    name: &'static str,
    kind: CompletionItemKind,
    extension: &'static str,
    description: &'static str,
}

const BUILTIN_COMPLETIONS: [BuiltinCompletionItem; 21] = [
    BuiltinCompletionItem {
        name: "int",
        kind: CompletionItemKind::KEYWORD,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "64-bit, two's complement, signed integers.",
    },
    BuiltinCompletionItem {
        name: "bool",
        kind: CompletionItemKind::KEYWORD,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "True or false.",
    },
    BuiltinCompletionItem {
        name: "add",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "x + y.",
    },
    BuiltinCompletionItem {
        name: "mul",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "x + y.",
    },
    BuiltinCompletionItem {
        name: "sub",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "x × y.",
    },
    BuiltinCompletionItem {
        name: "div",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "x ÷ y. It is an error to `div` by zero.",
    },
    BuiltinCompletionItem {
        name: "eq",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "Equal.",
    },
    BuiltinCompletionItem {
        name: "lt",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "Less than.",
    },
    BuiltinCompletionItem {
        name: "gt",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "Greater than.",
    },
    BuiltinCompletionItem {
        name: "le",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "Less than or equal to.",
    },
    BuiltinCompletionItem {
        name: "ge",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "Greater than or equal to.",
    },
    BuiltinCompletionItem {
        name: "not",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "(1 argument)",
    },
    BuiltinCompletionItem {
        name: "and",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "(2 arguments)",
    },
    BuiltinCompletionItem {
        name: "or",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "(2 arguments)",
    },
    BuiltinCompletionItem {
        name: "jmp",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "Unconditional jump. One label: the label to jump to.",
    },
    BuiltinCompletionItem {
        name: "br",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "Conditional branch. One argument: a variable of type `bool`. Two labels: a true label and a false label. Transfer control to one of the two labels depending on the value of the variable."
    },
    BuiltinCompletionItem {
        name: "call",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "`call`: Function invocation. Takes the name of the function to call and, as its arguments, the function parameters. The `call` instruction can be a Value Operation or an Effect Operation, depending on whether the function returns a value.\n\nOnly `call` may (optionally) produce a result; the rest [of the control-flow operations] appear only as Effect Operations.",
    },
    BuiltinCompletionItem {
        name: "ret",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "Function return. Stop executing the current activation record and return to the parent (or exit the program if this is the top-level main activation record). It has one optional argument: the return value for the function.",
    },
    BuiltinCompletionItem {
        name: "id",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "A type-insensitive identity. Takes one argument, which is a variable of any type, and produces the same value (which must have the same type, obvi).",
    },
    BuiltinCompletionItem {
        name: "print",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "Output values to the console (with a newline). Takes any number of arguments of any type and does not produce a result.",
    },
    BuiltinCompletionItem {
        name: "nop",
        kind: CompletionItemKind::FUNCTION,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "Do nothing. Takes no arguments and produces no result.",
    },
];

const BUILTIN_COMPLETIONS_LENGTH: usize = BUILTIN_COMPLETIONS.len();

pub fn get_builtin_completions() -> [CompletionItem; BUILTIN_COMPLETIONS_LENGTH]
{
    BUILTIN_COMPLETIONS
        .iter()
        .map(|item| CompletionItem {
            label: item.name.to_string(),
            kind: Some(item.kind),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    "{}\n---\nExtension: <{}>",
                    item.description, item.extension
                ),
            })),
            ..Default::default()
        })
        .collect::<Vec<CompletionItem>>()
        .try_into()
        .unwrap()
}

#[derive(Debug)]
struct Backend {
    client: Client,
    files: Arc<DashMap<PathBuf, Program>>,
    builtin_completions: [CompletionItem; BUILTIN_COMPLETIONS_LENGTH],
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            files: Arc::new(DashMap::new()),
            builtin_completions: get_builtin_completions(),
        }
    }

    async fn compile(
        &self,
        message: &'static str,
        uri: Url,
        version: Option<i32>,
    ) {
        let Ok(path) = PathBuf::from(uri.path()).canonicalize() else {
            self.client
                .log_message(
                    MessageType::ERROR,
                    format!("{}: failed to canonicalize {}", message, uri),
                )
                .await;
            return;
        };

        self.client
            .log_message(
                MessageType::LOG,
                format!("{}: {}", message, path.to_string_lossy()),
            )
            .await;

        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) => {
                self.client
                    .publish_diagnostics(
                        uri,
                        vec![Diagnostic::new(
                            Range::new(
                                Position::new(0, 0),
                                Position::new(0, 0),
                            ),
                            Some(DiagnosticSeverity::ERROR),
                            None,
                            None,
                            format!(
                                "Failed to open file {}: {}",
                                path.to_string_lossy(),
                                error
                            ),
                            None,
                            None,
                        )],
                        version,
                    )
                    .await;
                return;
            }
        };

        let program: Program = match serde_json::from_str(&contents) {
            Ok(program) => program,
            Err(error) => {
                let position = Position::new(
                    error.line() as u32 - 1,
                    error.column() as u32 - 1,
                );
                self.client
                    .publish_diagnostics(
                        uri,
                        vec![Diagnostic::new(
                            Range::new(position, position),
                            Some(DiagnosticSeverity::ERROR),
                            None,
                            None,
                            format!(
                                "Failed to parse program {}: {}",
                                path.to_string_lossy(),
                                error
                            ),
                            None,
                            None,
                        )],
                        version,
                    )
                    .await;
                return;
            }
        };

        // let basic_block_program = match BBProgram::new(program) {
        //     Ok(basic_block_program) => basic_block_program,
        //     Err(error) => {
        //         self.client
        //             .publish_diagnostics(
        //                 uri,
        //                 vec![Diagnostic::new(
        //                     Range::new(
        //                         Position::new(0, 0),
        //                         Position::new(0, 0),
        //                     ),
        //                     Some(DiagnosticSeverity::ERROR),
        //                     None,
        //                     None,
        //                     format!(
        //                         "Failed to build basic block program from {}: {}",
        //                         path.to_string_lossy(),
        //                         error
        //                     ),
        //                     None,
        //                     None,
        //                 )],
        //                 version
        //             )
        //             .await;
        //         return;
        //     }
        // };

        // if let Err(error) = check::type_check(&basic_block_program) {
        //     let error_message = error.to_string();
        //     let position = error
        //         .pos
        //         .as_ref()
        //         .map(|pos| {
        //             Position::new(
        //                 pos.pos.row as u32 - 1,
        //                 pos.pos.col as u32 - 1,
        //             )
        //         })
        //         .unwrap_or(Position::new(0, 0));
        //     self.client
        //         .publish_diagnostics(
        //             uri,
        //             vec![Diagnostic::new(
        //                 Range::new(position, position),
        //                 Some(DiagnosticSeverity::ERROR),
        //                 None,
        //                 None,
        //                 format!("Program failed to type check",),
        //                 None,
        //                 None,
        //             )],
        //             version,
        //         )
        //         .await;
        //     return;
        // }

        self.files.insert(path, program);
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        _: InitializeParams,
    ) -> jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions::default()),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.compile(
            "did_open",
            params.text_document.uri,
            Some(params.text_document.version),
        )
        .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.compile(
            "did_change",
            params.text_document.uri,
            Some(params.text_document.version),
        )
        .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.compile("did_save", params.text_document.uri, None)
            .await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }

    async fn completion(
        &self,
        _: CompletionParams,
    ) -> jsonrpc::Result<Option<CompletionResponse>> {
        Ok(Some(CompletionResponse::Array(
            self.builtin_completions.to_vec(),
        )))
    }

    async fn hover(
        &self,
        _hover: HoverParams,
    ) -> jsonrpc::Result<Option<Hover>> {
        Ok(None)
        // Ok(Some(Hover {
        //     contents: HoverContents::Scalar(MarkedString::String(
        //         "You're hovering!".to_string(),
        //     )),
        //     range: None,
        // }))
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
