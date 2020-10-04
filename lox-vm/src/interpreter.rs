use super::chunk::*;
use super::value::{FromValue, Object, ToValue, Value};
use std::collections::HashMap;

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
    heap: HashMap<usize, Object>,
}

impl VM {
    pub fn new() -> VM {
        VM {
            chunk: Chunk::new(),
            ip: 0,
            stack: vec![],
            heap: HashMap::new(),
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

    fn read_constant(&self, address: usize) -> Value {
        self.chunk.constants[address].clone()
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

    fn values_equal(&self, a: Value, b: Value) -> bool {
        match (a, b) {
            (Value::Boolean(ba), Value::Boolean(bb)) => ba == bb,
            (Value::Number(na), Value::Number(nb)) => na == nb,
            (Value::Nil, Value::Nil) => true,
            _ => false,
        }
    }

    fn print(&self, value: Value) {
        match value {
            Value::Number(n) => println!("{} : Number", n),
            Value::Boolean(b) => println!("{} : Boolean", b),
            Value::Object(p) => println!("{:?} : Object", self.heap[&p]),
            Value::Nil => println!("nil : Nil"),
        }
    }

    fn peek(&self, look_back: usize) -> &Value {
        &self.stack[self.stack.len() - 1 - look_back]
    }

    fn deref(&self, value: &Value) -> Option<&Object> {
        match value {
            Value::Object(p) => Some(&self.heap[p]),
            _ => None,
        }
    }

    fn add_to_heap(&mut self, object: Object) -> usize {
        let new_address = self.heap.len();
        self.heap.insert(new_address, object);
        new_address
    }

    fn concat_string(&mut self) -> Result<(), InterpreterError> {
        let b = self.pop();
        let a = self.pop();
        let obj_a = self.deref(&a);
        let obj_b = self.deref(&b);

        match (obj_a, obj_b) {
            (Some(Object::String(s_a)), Some(Object::String(s_b))) => {
                let s_new = format!("{}{}", s_a, s_b);
                let address = self.add_to_heap(Object::String(s_new));
                self.stack.push(Value::Object(address));
            }
            _ => panic!("Unreachable string concat"),
        }

        Ok(())
    }

    fn run(&mut self) -> Result<(), InterpreterError> {
        loop {
            match self.consume() {
                OpCode::Return => {
                    let value = self.pop();
                    self.print(value);
                    return Ok(());
                }
                OpCode::Constant(address) => {
                    let val = self.read_constant(address);
                    self.push(val);
                }
                OpCode::Hoist => {
                    //Lifo order to match the order the op codes were added.
                    let object = self.chunk.heap_hoist.remove(0);
                    let address = self.add_to_heap(object);
                    self.stack.push(Value::Object(address))
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
                    let obj_1 = self.deref(self.peek(0));
                    let obj_2 = self.deref(self.peek(1));
                    match (obj_1, obj_2) {
                        (Some(Object::String(_)), Some(Object::String(_))) => {
                            self.concat_string()?
                        }
                        _ => self.binary_op(|a: f64, b: f64| a + b)?,
                    };
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
                    let result = self.values_equal(a, b);
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
