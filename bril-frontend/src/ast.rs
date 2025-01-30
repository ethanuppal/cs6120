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

use crate::loc::Loc;

pub struct Program<'a> {
    pub imports: Vec<Loc<Import<'a>>>,
    pub functions: Vec<Loc<Function<'a>>>,
}

pub struct ImportedFunctionAlias<'a> {
    pub as_token: Loc<()>,
    pub name: Loc<&'a str>,
}

pub struct ImportedFunction<'a> {
    pub name: Loc<&'a str>,
    pub alias: Option<Loc<ImportedFunctionAlias<'a>>>,
}

pub struct Import<'a> {
    pub from_token: Loc<()>,
    pub path: Loc<&'a str>,
    pub import_token: Loc<()>,
    pub imported_functions: Vec<Loc<ImportedFunction<'a>>>,
}

pub struct Function<'a> {
    pub name: Loc<&'a str>,
    pub parameters: Vec<(Loc<&'a str>, Loc<TypeAnnotation>)>,
    pub return_type: Option<Loc<TypeAnnotation>>,
    pub body: Vec<Loc<FunctionCode<'a>>>,
}

pub enum FunctionCode<'a> {
    Label {
        label: Loc<Label<'a>>,
        colon_token: Loc<()>,
    },
    Instruction(Loc<Instruction<'a>>),
}

pub struct Label<'a> {
    pub name: Loc<&'a str>,
}

pub enum Type {
    Int,
    Float,
    Char,
    Ptr(Box<Loc<Type>>),
}

pub struct TypeAnnotation {
    pub colon_token: Loc<()>,
    pub ty: Loc<Type>,
}

pub enum Instruction<'a> {
    Constant(Constant<'a>),
    Value(ValueOperation<'a>),
    Effect(EffectOperation<'a>),
}

pub enum ConstantValue {
    IntLiteral(Loc<i64>),
    FloatLiteral(Loc<f64>),
    CharLiteral(Loc<char>),
}

pub struct Constant<'a> {
    pub name: Loc<&'a str>,
    pub type_annotation: Option<Loc<TypeAnnotation>>,
    pub equals_token: Loc<()>,
    pub value: Loc<ConstantValue>,
}

pub enum ValueOperationOp<'a> {
    Add(Loc<&'a str>, Loc<&'a str>),
    Mul(Loc<&'a str>, Loc<&'a str>),
    Sub(Loc<&'a str>, Loc<&'a str>),
    Div(Loc<&'a str>, Loc<&'a str>),

    Eq(Loc<&'a str>, Loc<&'a str>),
    Lt(Loc<&'a str>, Loc<&'a str>),
    Gt(Loc<&'a str>, Loc<&'a str>),
    Le(Loc<&'a str>, Loc<&'a str>),
    Ge(Loc<&'a str>, Loc<&'a str>),

    Not(Loc<&'a str>),
    And(Loc<&'a str>, Loc<&'a str>),
    Or(Loc<&'a str>, Loc<&'a str>),

    Id(Loc<&'a str>),
}

pub struct ValueOperation<'a> {
    pub name: Loc<&'a str>,
    pub type_annotation: Option<Loc<TypeAnnotation>>,
    pub equals_token: Loc<()>,
    pub op: Loc<ValueOperationOp<'a>>,
}

pub enum EffectOperation<'a> {
    Jmp(Loc<Label<'a>>),
    Br(Loc<Label<'a>>, Loc<Label<'a>>),
    Call(Loc<&'a str>, Vec<Loc<&'a str>>),
    Ret,

    Print(Vec<Loc<&'a str>>),
    Nop,
}
