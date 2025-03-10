use llvm_plugin::{
    LlvmModulePass, PassBuilder, PipelineParsing, PreservedAnalyses,
};

#[llvm_plugin::plugin(name = "CustomPass", version = "0.1")]
fn plugin_registrar(builder: &mut PassBuilder) {
    builder.add_module_pipeline_parsing_callback(|name, manager| {
        if name == "custom-pass" {
            manager.add_pass(CustomPass);
            PipelineParsing::Parsed
        } else {
            PipelineParsing::NotParsed
        }
    });
}

struct CustomPass;

impl LlvmModulePass for CustomPass {
    fn run_pass(
        &self,
        module: &mut llvm_plugin::inkwell::module::Module<'_>,
        manager: &llvm_plugin::ModuleAnalysisManager,
    ) -> PreservedAnalyses {
        for function in module.get_functions() {
            eprintln!("Hello from: {:?}", function.get_name());
            eprintln!("  number of arguments: {}", function.count_params());
        }
        PreservedAnalyses::All
    }
}
