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

use std::path::PathBuf;

use tower_lsp::{
    jsonrpc, lsp_types::*, Client, LanguageServer, LspService, Server,
};

struct BuiltinCompletionItem {
    name: &'static str,
    kind: CompletionItemKind,
    extension: &'static str,
    description: &'static str,
}

const BUILTIN_COMPLETIONS: [BuiltinCompletionItem; 1] =
    [BuiltinCompletionItem {
        name: "int",
        kind: CompletionItemKind::KEYWORD,
        extension: "https://capra.cs.cornell.edu/bril/lang/core.html",
        description: "64-bit, two's complement, signed integers.",
    }];

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
                    "{}\n\nExtension: <{}>",
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
    builtin_completions: [CompletionItem; BUILTIN_COMPLETIONS_LENGTH],
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            builtin_completions: get_builtin_completions(),
        }
    }

    async fn compile(&self, message: &str, path: String) {
        let Ok(path) = PathBuf::from(&path).canonicalize() else {
            self.client
                .log_message(
                    MessageType::ERROR,
                    format!("{}: failed to canonicalize {}", message, path),
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

        // TODO: compile
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
        self.compile("did_open", params.text_document.uri.path().to_string())
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.compile("did_change", params.text_document.uri.path().to_string())
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.compile("did_save", params.text_document.uri.path().to_string())
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
