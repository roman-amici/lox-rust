use super::value::Value;

#[derive(Debug, Copy, Clone)]
pub enum OpCode {
    Constant(usize), //Index into the constants array
    DefineGlobal(usize),
    Nil,
    True,
    False,
    Negate,
    Add,
    Subtract,
    Multiply,
    Divide,
    Return,
    Print,
    Pop,
    Not,
    Equal,
    Greater,
    Less,
    GetGlobal(usize),
    SetGlobal(usize),
    SetLocal(usize),
    GetLocal(usize),
    GetUpValue(usize),
    SetUpValue(usize),
    JumpIfFalse(usize),
    Jump(usize),
    Loop(usize), //Backwards offset instead of forward
    Call(usize),
    Closure(usize, usize), // (Constant pointer, number of upvalues)
    Class(usize),
    Upvalue(Upvalue),
    SetProperty(usize), //Constant index for name
    GetProperty(usize),
    CloseUpvalue,
    Method(usize), //Constant index for name
    ThisPlaceholder,
    EOF,
}

#[derive(Debug, Copy, Clone)]
pub struct Upvalue {
    pub is_local: bool,
    pub index: usize, //Index in local slots
}

#[derive(Clone)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    pub constants: Vec<Value>,
    pub line_numbers: Vec<usize>,
}

impl Chunk {
    pub fn new() -> Chunk {
        Chunk {
            code: vec![],
            constants: vec![],
            line_numbers: vec![],
        }
    }

    pub fn add_constant(&mut self, constant: Value) -> usize {
        self.constants.push(constant);
        self.constants.len() - 1
    }

    pub fn append_chunk(&mut self, op: OpCode, line: usize) -> usize {
        self.code.push(op);
        self.line_numbers.push(line);
        self.code.len() - 1
    }

    pub fn patch_jump(&mut self, instruction_idx: usize, offset: usize) {
        match &mut self.code[instruction_idx] {
            OpCode::JumpIfFalse(j) | OpCode::Jump(j) => *j = offset,
            _ => panic!(format!(
                "Cant patch opcode {:?}",
                self.code[instruction_idx]
            )),
        };
    }

    pub fn next(&self) -> usize {
        self.code.len()
    }

    pub fn top(&self) -> usize {
        self.code.len() - 1
    }
}
