use super::value::{Object, Value};

#[derive(Debug, Copy, Clone)]
pub enum OpCode {
    Constant(usize), //Index into the constants array
    Hoist,
    Nil,
    True,
    False,
    Negate,
    Add,
    Subtract,
    Multiply,
    Divide,
    Return,
    Not,
    Equal,
    Greater,
    Less,
}

#[derive(Clone)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    pub constants: Vec<Value>,
    pub line_numbers: Vec<usize>,
    pub heap_hoist: Vec<Object>,
}

impl Chunk {
    pub fn new() -> Chunk {
        Chunk {
            code: vec![],
            constants: vec![],
            line_numbers: vec![],
            heap_hoist: vec![],
        }
    }

    pub fn add_constant(&mut self, constant: Value) -> usize {
        self.constants.push(constant);
        self.constants.len() - 1
    }

    pub fn append_chunk(&mut self, op: OpCode, line: usize) {
        self.code.push(op);
        self.line_numbers.push(line);
    }
}
