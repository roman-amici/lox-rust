use super::chunk::*;
use super::value::{FromValue, ToValue, Value};

pub enum InterpreterError {
    TypeError(usize, String),
}

impl InterpreterError {
    pub fn to_string(&self) -> String {
        let InterpreterError::TypeError(line, msg) = self;
        format!("{}: {}", line, msg)
    }
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

    #[inline]
    fn current_line(&self) -> usize {
        //Since we've already advanced past it
        self.chunk.line_numbers[self.ip - 1]
    }

    fn lox_bool_coercion(val: Value) -> bool {
        match val {
            Value::Boolean(b) => b,
            Value::Nil => false,
            _ => true,
        }
    }

    fn binary_op<T1: FromValue, T2: FromValue, R: ToValue>(
        &mut self,
        bop: fn(T1, T2) -> R,
    ) -> Result<(), InterpreterError> {
        let b = T2::as_val(self.pop(), self.current_line())?;
        let a = T1::as_val(self.pop(), self.current_line())?;
        let result = R::to_value(bop(a, b));
        self.push(result);
        Ok(())
    }

    fn values_equal(a: Value, b: Value) -> bool {
        match (a, b) {
            (Value::Boolean(ba), Value::Boolean(bb)) => ba == bb,
            (Value::Number(na), Value::Number(nb)) => na == nb,
            (Value::Nil, Value::Nil) => true,
            _ => false,
        }
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
                OpCode::Negate => match self.pop() {
                    Value::Number(n) => self.push(Value::Number(-n)),
                    _ => {
                        return Err(InterpreterError::TypeError(
                            self.current_line(),
                            String::from("Operand must be a number."),
                        ))
                    }
                },
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
                OpCode::Nil => {
                    self.stack.push(Value::Nil);
                }
                OpCode::True => self.stack.push(Value::Boolean(true)),
                OpCode::False => self.stack.push(Value::Boolean(false)),
                OpCode::Not => {
                    let b = VM::lox_bool_coercion(self.pop());
                    self.stack.push(Value::Boolean(!b));
                }
                OpCode::Equal => {
                    let b = self.pop();
                    let a = self.pop();
                    let result = VM::values_equal(a, b);
                    self.stack.push(Value::Boolean(result));
                }
                OpCode::Greater => {
                    self.binary_op(|a: f64, b: f64| a > b)?;
                }
                OpCode::Less => {
                    self.binary_op(|a: f64, b: f64| a < b)?;
                }
            }
        }
    }
}
