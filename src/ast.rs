use crate::error::SourceLocation;

/// A complete AWK program
#[derive(Debug, Clone)]
pub struct Program {
    pub rules: Vec<Rule>,
    pub functions: Vec<FunctionDef>,
}

impl Program {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            functions: Vec::new(),
        }
    }
}

impl Default for Program {
    fn default() -> Self {
        Self::new()
    }
}

/// A pattern-action rule
#[derive(Debug, Clone)]
pub struct Rule {
    pub pattern: Option<Pattern>,
    pub action: Option<Block>,
    pub location: SourceLocation,
}

/// Pattern types that can trigger a rule
#[derive(Debug, Clone)]
pub enum Pattern {
    /// BEGIN - runs before any input
    Begin,
    /// END - runs after all input
    End,
    /// BEGINFILE - runs at the start of each input file (gawk extension)
    BeginFile,
    /// ENDFILE - runs at the end of each input file (gawk extension)
    EndFile,
    /// Expression that evaluates to true/false
    Expr(Expr),
    /// Regex pattern: /pattern/
    Regex(String),
    /// Range pattern: /start/,/end/
    Range {
        start: Box<Pattern>,
        end: Box<Pattern>,
    },
    /// Compound patterns with logical operators
    And(Box<Pattern>, Box<Pattern>),
    Or(Box<Pattern>, Box<Pattern>),
    Not(Box<Pattern>),
}

/// User-defined function
#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name: String,
    pub params: Vec<String>,
    pub body: Block,
    pub location: SourceLocation,
}

/// A block of statements
#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub location: SourceLocation,
}

impl Block {
    pub fn new(statements: Vec<Stmt>, location: SourceLocation) -> Self {
        Self {
            statements,
            location,
        }
    }

    pub fn empty(location: SourceLocation) -> Self {
        Self {
            statements: Vec::new(),
            location,
        }
    }
}

/// Statement types
#[derive(Debug, Clone)]
pub enum Stmt {
    /// Expression statement (e.g., function call, assignment)
    Expr(Expr),

    /// Print statement: print expr, expr, ...
    Print {
        args: Vec<Expr>,
        output: Option<OutputRedirect>,
        location: SourceLocation,
    },

    /// Printf statement: printf format, expr, ...
    Printf {
        format: Expr,
        args: Vec<Expr>,
        output: Option<OutputRedirect>,
        location: SourceLocation,
    },

    /// If statement
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
        location: SourceLocation,
    },

    /// While loop
    While {
        condition: Expr,
        body: Box<Stmt>,
        location: SourceLocation,
    },

    /// Do-while loop
    DoWhile {
        body: Box<Stmt>,
        condition: Expr,
        location: SourceLocation,
    },

    /// For loop (C-style)
    For {
        init: Option<Box<Stmt>>,
        condition: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
        location: SourceLocation,
    },

    /// For-in loop (array iteration)
    ForIn {
        var: String,
        array: String,
        body: Box<Stmt>,
        location: SourceLocation,
    },

    /// Block of statements
    Block(Block),

    /// Break statement
    Break { location: SourceLocation },

    /// Continue statement
    Continue { location: SourceLocation },

    /// Next statement (skip to next record)
    Next { location: SourceLocation },

    /// Nextfile statement (skip to next file)
    Nextfile { location: SourceLocation },

    /// Exit statement
    Exit {
        code: Option<Expr>,
        location: SourceLocation,
    },

    /// Return statement
    Return {
        value: Option<Expr>,
        location: SourceLocation,
    },

    /// Delete statement: delete array\[index\]
    Delete {
        array: String,
        index: Vec<Expr>,
        location: SourceLocation,
    },

    /// Empty statement (just a semicolon)
    Empty,
}

/// Output redirection for print/printf
#[derive(Debug, Clone)]
pub enum OutputRedirect {
    /// > file
    Truncate(Expr),
    /// >> file
    Append(Expr),
    /// | command
    Pipe(Expr),
}

/// Input source for getline
#[derive(Debug, Clone)]
pub enum GetlineInput {
    /// < file
    File(Box<Expr>),
    /// command |
    Pipe(Box<Expr>),
}

/// Expression types
#[derive(Debug, Clone)]
pub enum Expr {
    /// Numeric literal
    Number(f64, SourceLocation),

    /// String literal
    String(String, SourceLocation),

    /// Regex literal
    Regex(String, SourceLocation),

    /// Variable reference
    Var(String, SourceLocation),

    /// Field access: $expr
    Field(Box<Expr>, SourceLocation),

    /// Array access: arr\[index\] or arr\[i,j\]
    ArrayAccess {
        array: String,
        indices: Vec<Expr>,
        location: SourceLocation,
    },

    /// Binary operation
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        location: SourceLocation,
    },

    /// Unary operation
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        location: SourceLocation,
    },

    /// Assignment
    Assign {
        target: Box<Expr>,
        op: AssignOp,
        value: Box<Expr>,
        location: SourceLocation,
    },

    /// Pre-increment: ++x
    PreIncrement(Box<Expr>, SourceLocation),

    /// Pre-decrement: --x
    PreDecrement(Box<Expr>, SourceLocation),

    /// Post-increment: x++
    PostIncrement(Box<Expr>, SourceLocation),

    /// Post-decrement: x--
    PostDecrement(Box<Expr>, SourceLocation),

    /// Ternary: cond ? then : else
    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        location: SourceLocation,
    },

    /// Function call
    Call {
        name: String,
        args: Vec<Expr>,
        location: SourceLocation,
    },

    /// Array membership: (key) in array
    InArray {
        key: Vec<Expr>,
        array: String,
        location: SourceLocation,
    },

    /// Regex match: expr ~ /regex/ or expr !~ /regex/
    Match {
        expr: Box<Expr>,
        pattern: Box<Expr>,
        negated: bool,
        location: SourceLocation,
    },

    /// Getline as expression (returns status)
    Getline {
        /// Target lvalue (variable, array element, or field); `None` sets $0.
        var: Option<Box<Expr>>,
        input: Option<GetlineInput>,
        location: SourceLocation,
    },

    /// Grouping parentheses (for precedence)
    Group(Box<Expr>, SourceLocation),
}

impl Expr {
    pub fn location(&self) -> SourceLocation {
        match self {
            Expr::Number(_, loc)
            | Expr::String(_, loc)
            | Expr::Regex(_, loc)
            | Expr::Var(_, loc)
            | Expr::Field(_, loc)
            | Expr::ArrayAccess { location: loc, .. }
            | Expr::Binary { location: loc, .. }
            | Expr::Unary { location: loc, .. }
            | Expr::Assign { location: loc, .. }
            | Expr::PreIncrement(_, loc)
            | Expr::PreDecrement(_, loc)
            | Expr::PostIncrement(_, loc)
            | Expr::PostDecrement(_, loc)
            | Expr::Ternary { location: loc, .. }
            | Expr::Call { location: loc, .. }
            | Expr::InArray { location: loc, .. }
            | Expr::Match { location: loc, .. }
            | Expr::Getline { location: loc, .. }
            | Expr::Group(_, loc) => *loc,
        }
    }
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,

    // Comparison
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,

    // Logical
    And,
    Or,

    // String
    Concat,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg, // -x
    Pos, // +x
    Not, // !x
}

/// Assignment operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,    // =
    AddAssign, // +=
    SubAssign, // -=
    MulAssign, // *=
    DivAssign, // /=
    ModAssign, // %=
    PowAssign, // ^=
}
