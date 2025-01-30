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

use std::fmt::{self, Write};

use inform::common::IndentWriterCommon;

use crate::ast;

pub struct Printer<'source, 'writer, W: fmt::Write> {
    w: inform::fmt::IndentWriter<'writer, W>,
    source: &'source str,
}

impl<'source, 'writer, W: fmt::Write> Printer<'source, 'writer, W> {
    pub fn new(
        source: &'source str,
        writer: &'writer mut W,
        indent: usize,
    ) -> Self {
        Self {
            source,
            w: inform::fmt::IndentWriter::new(writer, indent),
        }
    }

    pub fn print_imported_function_alias(
        &mut self,
        imported_function_alias: &ast::ImportedFunctionAlias,
    ) -> fmt::Result {
        write!(self.w, "as {}", imported_function_alias.name)
    }

    pub fn print_imported_function(
        &mut self,
        imported_function: &ast::ImportedFunction,
    ) -> fmt::Result {
        write!(self.w, "{}", imported_function.name)?;

        if let Some(alias) = &imported_function.alias {
            write!(self.w, " ")?;
            self.print_imported_function_alias(alias)?;
        }

        Ok(())
    }

    pub fn print_import(&mut self, import: &ast::Import) -> fmt::Result {
        write!(self.w, "from \"{}\" import ", import.path)?;
        for (i, imported_function) in
            import.imported_functions.iter().enumerate()
        {
            if i > 0 {
                write!(self.w, ", ")?;
            }
            self.print_imported_function(imported_function)?;
        }
        writeln!(self.w)?;

        Ok(())
    }

    pub fn print_type(&mut self, ty: &ast::Type) -> fmt::Result {
        match ty {
            ast::Type::Int => write!(self.w, "int"),
            ast::Type::Float => write!(self.w, "float"),
            ast::Type::Char => write!(self.w, "char"),
            ast::Type::Ptr(inner) => {
                write!(self.w, "ptr<")?;
                self.print_type(inner)?;
                write!(self.w, ">")
            }
        }
    }

    pub fn print_type_annotation(
        &mut self,
        type_annotation: &ast::TypeAnnotation,
    ) -> fmt::Result {
        write!(self.w, ": ")?;
        self.print_type(&type_annotation.ty)
    }

    pub fn print_label(&mut self, label: &ast::Label) -> fmt::Result {
        write!(self.w, "{}", label.name)
    }

    pub fn print_constant_value(
        &mut self,
        constant_value: &ast::ConstantValue,
    ) -> fmt::Result {
        match constant_value {
            ast::ConstantValue::IntegerLiteral(integer) => {
                write!(self.w, "{}", integer)
            }
            ast::ConstantValue::FloatLiteral(float) => {
                write!(self.w, "{}", float)
            }
            ast::ConstantValue::CharacterLiteral(character) => {
                write!(self.w, "{}", character)
            }
        }
    }

    pub fn print_constant(&mut self, constant: &ast::Constant) -> fmt::Result {
        write!(self.w, "{}", constant.name)?;
        if let Some(type_annotation) = &constant.type_annotation {
            self.print_type_annotation(type_annotation)?;
        }
        write!(self.w, " = const ")?;
        self.print_constant_value(&constant.value)?;
        writeln!(self.w, ";")
    }

    pub fn print_instruction(
        &mut self,
        instruction: &ast::Instruction,
    ) -> fmt::Result {
        match instruction {
            ast::Instruction::Constant(constant) => {
                self.print_constant(constant)
            }
            ast::Instruction::ValueOperation(loc) => todo!(),
            ast::Instruction::EffectOperation(loc) => todo!(),
        }
    }

    pub fn print_function_code(
        &mut self,
        code: &ast::FunctionCode,
    ) -> fmt::Result {
        match code {
            ast::FunctionCode::Label { label, .. } => {
                self.w.decrease_indent();
                self.print_label(label)?;
                writeln!(self.w, ":")?;
                self.w.increase_indent();
                Ok(())
            }
            ast::FunctionCode::Instruction(instruction) => {
                self.print_instruction(instruction)
            }
        }
    }

    pub fn print_function(&mut self, function: &ast::Function) -> fmt::Result {
        write!(self.w, "{}(", function.name)?;

        for (i, (name, type_annotation)) in
            function.parameters.iter().enumerate()
        {
            if i > 0 {
                write!(self.w, ", ")?;
            }
            write!(self.w, "{}", name)?;
            self.print_type_annotation(type_annotation)?;
        }

        write!(self.w, ")")?;

        if let Some(return_type) = &function.return_type {
            self.print_type_annotation(return_type)?;
        }

        writeln!(self.w, " {{")?;
        self.w.increase_indent();

        for code in &function.body {
            self.print_function_code(code)?;
        }

        self.w.decrease_indent();
        writeln!(self.w, "}}")?;

        Ok(())
    }

    pub fn print_program(&mut self, program: &ast::Program) -> fmt::Result {
        for import in &program.imports {
            self.print_import(import)?;
        }

        for function in &program.functions {
            self.print_function(function);
        }

        Ok(())
    }
}
