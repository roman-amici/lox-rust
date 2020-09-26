use super::value::Value;

#[derive(Debug, Copy, Clone)]
pub enum OpCode {
    Constant(usize), //Index into the constants array
    Negate,
    Add,
    Subtract,
    Multiply,
    Divide,
    Return,
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

    pub fn append_chunk(&mut self, op: OpCode, line: usize) {
        self.code.push(op);
        self.line_numbers.push(line);
    }
}
