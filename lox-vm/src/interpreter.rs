use super::chunk::*;
use super::value::Value;

pub enum InterpreterError {
    CompileError,
    RuntimeError,
}

pub struct VM {
    chunk: Chunk, //Current chunk to be executed. Takes ownership of the chunk during execution
    ip: usize,    // Instruction pointer
    stack: Vec<Value>,
}

impl VM {
    pub fn new() -> VM {
        VM {
            chunk: Chunk::new(),
            ip: 0,
            stack: vec![],
        }
    }

    pub fn interpret(&mut self, chunk: Chunk) -> Result<(), InterpreterError> {
        self.chunk = chunk;
        self.ip = 0;
        self.run()
    }

    #[inline]
    fn consume(&mut self) -> OpCode {
        let op = self.chunk.code[self.ip];
        self.ip += 1;
        op
    }

    #[inline]
    fn read_constant(&self, address: usize) -> Value {
        self.chunk.constants[address]
    }

    #[inline]
    fn push(&mut self, val: Value) {
        self.stack.push(val);
    }

    #[inline]
    fn pop(&mut self) -> Value {
        self.stack.pop().unwrap()
    }

    //TODO: Replace with macro
    fn binary_op(&mut self, bop: fn(f64, f64) -> f64) -> Result<(), InterpreterError> {
        let b = self.pop().as_number()?;
        let a = self.pop().as_number()?;
        let result = Value::Number(bop(a, b));
        self.push(result);
        Ok(())
    }

    fn run(&mut self) -> Result<(), InterpreterError> {
        loop {
            match self.consume() {
                OpCode::Return => {
                    println!("{:?}", self.pop());
                    return Ok(());
                }
                OpCode::Constant(address) => {
                    let val = self.read_constant(address);
                    self.push(val);
                }
                OpCode::Negate => {
                    match self.pop() {
                        Value::Number(n) => self.push(Value::Number(-n)),
                        _ => return Err(InterpreterError::RuntimeError), //TODO: fill this in
                    }
                }
                OpCode::Add => {
                    self.binary_op(|a: f64, b: f64| a + b)?;
                }
                OpCode::Subtract => {
                    self.binary_op(|a: f64, b: f64| a - b)?;
                }
                OpCode::Multiply => {
                    self.binary_op(|a: f64, b: f64| a * b)?;
                }
                OpCode::Divide => {
                    self.binary_op(|a: f64, b: f64| a / b)?;
                }
            }
        }
    }
}
