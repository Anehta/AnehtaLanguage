use anehta_lexer::Span;

/// Top-level program: a list of statements
#[derive(Debug)]
pub struct Program {
    pub statements: Vec<Statement>,
}

/// Statement types
#[derive(Debug)]
pub enum Statement {
    FuncDecl(FuncDecl),
    VarDecl(VarDecl),
    Assignment(Assignment),
    IfStmt(IfStmt),
    ForStmt(ForStmt),
    Block(Block),
    CallFunc(CallFunc),
    Return(ReturnStmt),
    Break(Span),
    Continue(Span),
    TimerStmt(TimerStmt),
    FieldAssign(FieldAssign),
    IndexAssign(IndexAssign),
    MethodCall(MethodCall),
}

/// Timer block: timer { body } — auto-measures and prints elapsed time
#[derive(Debug)]
pub struct TimerStmt {
    pub body: Block,
    pub span: Span,
}

/// Function declaration: func name(params) -> return_types { body }
#[derive(Debug)]
pub struct FuncDecl {
    pub name: String,
    pub params: Vec<FuncParam>,
    pub return_types: Vec<String>,
    pub body: Block,
    pub span: Span,
}

/// Function parameter: name: type
#[derive(Debug)]
pub struct FuncParam {
    pub name: String,
    pub type_name: String,
    pub span: Span,
}

/// Variable declaration: var name: type  OR  var name = expr
#[derive(Debug)]
pub enum VarDecl {
    TypeDecl {
        name: String,
        type_name: String,
        span: Span,
    },
    Assignment(Assignment),
}

/// Assignment: name1, name2 = expr1, expr2
#[derive(Debug)]
pub struct Assignment {
    pub targets: Vec<String>,
    pub values: Vec<Expr>,
    pub span: Span,
}

/// If statement: if (cond) { block } elseif ... else ...
#[derive(Debug)]
pub struct IfStmt {
    pub condition: BooleanExpr,
    pub body: Block,
    pub else_if: Vec<ElseIfBranch>,
    pub else_body: Option<Block>,
    pub span: Span,
}

#[derive(Debug)]
pub struct ElseIfBranch {
    pub condition: BooleanExpr,
    pub body: Block,
    pub span: Span,
}

/// For statement: for (init; cond; step) { body }
#[derive(Debug)]
pub struct ForStmt {
    pub init: Option<Box<Statement>>,
    pub condition: Option<BooleanExpr>,
    pub step: Option<Box<Statement>>,
    pub body: Block,
    pub span: Span,
}

/// Block: { statements }
#[derive(Debug)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

/// Function call: name(args)
#[derive(Debug)]
pub struct CallFunc {
    pub name: String,
    pub args: Vec<Expr>,
    pub span: Span,
}

/// Return statement: return expr1, expr2
#[derive(Debug)]
pub struct ReturnStmt {
    pub values: Vec<Expr>,
    pub span: Span,
}

/// Boolean expression (comparison with logical connectors)
#[derive(Debug)]
pub enum BooleanExpr {
    Comparison {
        left: Expr,
        op: ComparisonOp,
        right: Expr,
        span: Span,
    },
    Logical {
        left: Box<BooleanExpr>,
        op: LogicalOp,
        right: Box<BooleanExpr>,
        span: Span,
    },
    Grouped(Box<BooleanExpr>),
}

#[derive(Debug, Clone, Copy)]
pub enum ComparisonOp { Gt, Lt, GtEq, LtEq, Eq, NotEq }

#[derive(Debug, Clone, Copy)]
pub enum LogicalOp { And, Or }

/// Table literal: { key: value, ... }
#[derive(Debug)]
pub struct TableLiteral {
    pub entries: Vec<TableEntry>,
    pub span: Span,
}

/// A single key-value entry in a table literal
#[derive(Debug)]
pub struct TableEntry {
    pub key: String,
    pub value: Expr,
}

/// Field access: expr.field
#[derive(Debug)]
pub struct FieldAccess {
    pub object: Box<Expr>,
    pub field: String,
    pub span: Span,
}

/// Index access: expr["key"]
#[derive(Debug)]
pub struct IndexAccess {
    pub object: Box<Expr>,
    pub index: Box<Expr>,
    pub span: Span,
}

/// Field assignment: object.field = value
#[derive(Debug)]
pub struct FieldAssign {
    pub object: String,
    pub field: String,
    pub value: Expr,
    pub span: Span,
}

/// Index assignment: object["key"] = value
#[derive(Debug)]
pub struct IndexAssign {
    pub object: String,
    pub index: Expr,
    pub value: Expr,
    pub span: Span,
}

/// Method/indirect call: expr(args) — e.g. table.field(args)
#[derive(Debug)]
pub struct MethodCall {
    pub callee: Box<Expr>,
    pub args: Vec<Expr>,
    pub span: Span,
}

/// Closure parameter
#[derive(Debug)]
pub struct ClosureParam {
    pub name: String,
    pub type_name: Option<String>,
}

/// Closure body: single expression or block
#[derive(Debug)]
pub enum ClosureBody {
    Expr(Box<Expr>),
    Block(Block),
}

/// Closure expression: |params| => body
#[derive(Debug)]
pub struct ClosureExpr {
    pub params: Vec<ClosureParam>,
    pub body: ClosureBody,
    pub span: Span,
}

/// Arithmetic expression
#[derive(Debug)]
pub enum Expr {
    Number(String, Span),
    StringLit(String, Span),
    Bool(bool, Span),
    Variable(String, Span),
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
    UnaryOp {
        op: UnaryOp,
        operand: String,
        span: Span,
    },
    CallFunc(CallFunc),
    Closure(ClosureExpr),
    TableLiteral(TableLiteral),
    FieldAccess(FieldAccess),
    IndexAccess(IndexAccess),
    MethodCall(MethodCall),
    Grouped(Box<Expr>),
}

#[derive(Debug, Clone, Copy)]
pub enum BinaryOp { Add, Sub, Mul, Div, Power, Mod, Rand }

#[derive(Debug, Clone, Copy)]
pub enum UnaryOp { Increment, Decrement }
