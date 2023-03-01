use super::{Vec};

// NODES
// ================================================================================================

// TODO: Input/output
// TODO: Hashing, calls to stdlib, library imports
//
//Future TODOs:
//
//    Some yet-to-be-decided notion of native assets.
//    Some yet-to-be-decided notion of contracts+methods. This also includes blockchain network addresses, and Sway's notion of ContractID.
//    Some yet-to-be-decided notion of storage/state
//    Miden assembly blocks. Not obvious how to design this without it being variable-based.


pub enum Name{
    name: String,
}

// TODO: Add more integer types
// TODO: Add structs, enums, arrays, vectors, tuples, references, dereferencing
// TODO: Field integers added in later phase
// TODO: Byte strings/addresses/hashes
pub enum Type{
    U32,
    U64,
    Bool,
}

pub struct Var{
    name: String,MIx
    typ: Type,
}

// TODO: references, dereferencing
pub enum UnOpKind{
    LNeg,
}

pub enum BinOpKind{
    Add,
    Sub,
    Mul,
    Div, // Trucating division
    Rem, // Use for truncating casts
    Exp,
    Land,
    Lor,
    Lxor,
}

// Convention: Operands of a binary operation must be of the same type as the expected result type.
pub struct BinaryOp{
    BinOp: BinOpKind,
    lhs: Expression,
    rhs: Expression,
    typ: Type, // Expected result type
}

pub enum Pattern{
    Literal_u32(u32),
    Variable(Var),
    Wildcard
}

// Question: Add lambdas?
// TODO: Add structs, enums, arrays, vectors, tuples
pub enum Expression{
    Literal_u32(u32),
    VariableExpression(Var),
    UnOp(UnOpKind, Expression),
    BinOp(BinaryOp, Expression, Expression),
    AssertType(Type, Expression) , // Use for non-truncating casts, including upcasts
    MatchExpression(Expression, Vec<(Pattern, Vec<Expression>)>),
}

// Question: Add Goto and Labels?
pub enum Statement{
    Block(Vec<Statement>),
    While(Expression, Vec<Statement>),
    Break,
    Continue,
    IfElse(Expression, Vec<Statement>, Vec<Statement>),
    MatchStatement(Expression, Vec<(Pattern, Vec<Statement>)>),
    VariableDeclaration(Var, Expression),
    Assignment(Var, Expression),
    FunctionCall(Name, Vec<Expression>),
    ExpressionStatement(Expression)
}

pub enum Visibility{
    Public,
    Private,
}

pub struct FunctionDeclaration{
    name: Name,
    parameters: Vec<Var>,
    body: Vec<Statement>,
    visibility: Visibility,
}

pub struct ConstantDeclaration{
    name: Name,
    typ: Type,
    value: Expression
}

// Main (executable) function
pub struct BeginBlock{
    body: Vec<Statement>
}

pub struct Library{
    constants: Vec<ConstantDeclaration>,
    functions: Vec<FunctionDeclaration>,
}

pub struct Program{
    constants: Vec<ConstantDeclaration>,
    functions: Vec<FunctionDeclaration>,
    begin_block: BeginBlock,
}
