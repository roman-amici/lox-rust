use super::value::{Object, Value};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Copy, Clone)]
pub enum OpCode {
    Constant(usize), //Index into the constants array
    DefineGlobal(u64),
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
    GetGlobal(u64),
    SetGlobal(u64),
    SetLocal(usize),
    GetLocal(usize),
    EOF,
}

#[derive(Clone)]
pub struct Chunk {
    pub code: Vec<OpCode>,
    pub constants: Vec<Value>,
    pub line_numbers: Vec<usize>,
    pub new_strings: Vec<String>,
}

impl Chunk {
    pub fn new() -> Chunk {
        Chunk {
            code: vec![],
            constants: vec![],
            line_numbers: vec![],
            new_strings: vec![],
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

    pub fn add_string(&mut self, s: String) -> u64 {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        let hash_val = hasher.finish();
        self.new_strings.push(s);
        hash_val
    }
}
