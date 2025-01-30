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

    pub fn print_program(&mut self, program: &ast::Program) -> fmt::Result {
        for import in &program.imports {
            self.print_import(import)?;
        }

        Ok(())
    }
}
