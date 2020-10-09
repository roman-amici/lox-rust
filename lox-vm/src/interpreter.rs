use super::chunk::*;
use super::value::{FromValue, FromValueRef, Object, ToValue, Value};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::mem::swap;

pub enum InterpreterError {
    TypeError(usize, String),
    NameError(usize, String),
}

impl InterpreterError {
    pub fn to_string(&self) -> String {
        match self {
            InterpreterError::TypeError(line, msg) | InterpreterError::NameError(line, msg) => {
                format!("{}: {}", line, msg)
            }
        }
    }
}

pub struct VM {
    chunk: Chunk, //Current chunk to be executed. Takes ownership of the chunk during execution
    ip: usize,    // Instruction pointer
    stack: Vec<Value>,
    heap: HashMap<usize, Object>,
    globals: HashMap<u64, Value>,
    strings: HashMap<u64, String>,
}

impl VM {
    pub fn new() -> VM {
        VM {
            chunk: Chunk::new(),
            ip: 0,
            stack: vec![],
            heap: HashMap::new(),
            globals: HashMap::new(),
            strings: HashMap::new(),
        }
    }

    pub fn add_new_string(&mut self, s: String) -> u64 {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        let hash_val = hasher.finish();
        if self.strings.contains_key(&hash_val) && self.strings[&hash_val] != s {
            panic!("Hash collision!");
        } else {
            self.strings.insert(hash_val, s);
        }

        hash_val
    }

    pub fn interpret(&mut self, chunk: Chunk) -> Result<(), InterpreterError> {
        self.chunk = chunk;
        let mut new_strings: Vec<String> = vec![];
        swap(&mut self.chunk.new_strings, &mut new_strings);
        for s in new_strings {
            self.add_new_string(s);
        }

        self.ip = 0;
        self.run()
    }

    #[inline]
    fn consume(&mut self) -> OpCode {
        if self.ip < self.chunk.code.len() {
            let op = self.chunk.code[self.ip];
            self.ip += 1;
            op
        } else {
            OpCode::EOF
        }
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

    fn deref_str_ptr(&self, value: Value) -> Result<&String, InterpreterError> {
        match value {
            Value::StrPtr(p) => Ok(self.get_string(p)),
            _ => Err(InterpreterError::TypeError(
                0,
                String::from("Impossible Error: Expected String"),
            )),
        }
    }

    fn string_concat(&mut self) -> Result<(), InterpreterError> {
        let b = self.pop();
        let a = self.pop();
        let s_a = self.deref_str_ptr(a)?;
        let s_b = self.deref_str_ptr(b)?;

        let s_c = format!("{}{}", s_a, s_b);
        let p = self.add_new_string(s_c);
        self.stack.push(Value::StrPtr(p));
        Ok(())
    }

    fn values_equal(&self, a: Value, b: Value) -> bool {
        match (a, b) {
            (Value::Boolean(ba), Value::Boolean(bb)) => ba == bb,
            (Value::Number(na), Value::Number(nb)) => na == nb,
            (Value::Nil, Value::Nil) => true,
            (Value::StrPtr(p_a), Value::StrPtr(p_b)) => p_a == p_b,
            (Value::Object(p_a), Value::Object(p_b)) => {
                let v_a = self.follow(p_a);
                let v_b = self.follow(p_b);
                match (v_a, v_b) {
                    (Object::String(s1), Object::String(s2)) => s1 == s2,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn get_string(&self, p: u64) -> &String {
        &self.strings[&p]
    }

    fn print(&self, value: Value) {
        match value {
            Value::Number(n) => println!("{} : Number", n),
            Value::Boolean(b) => println!("{} : Boolean", b),
            Value::Object(p) => println!("{:?} : Object", self.heap[&p]),
            Value::Nil => println!("nil : Nil"),
            Value::StrPtr(p) => println!("{}", self.get_string(p)),
        }
    }

    fn peek(&self, look_back: usize) -> &Value {
        &self.stack[self.stack.len() - 1 - look_back]
    }

    fn follow(&self, pointer: usize) -> &Object {
        &self.heap[&pointer]
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

    fn run(&mut self) -> Result<(), InterpreterError> {
        loop {
            match self.consume() {
                OpCode::EOF => return Ok(()),
                OpCode::Return => {
                    let value = self.pop();
                    self.print(value);
                    return Ok(());
                }
                OpCode::Print => {
                    let value = self.pop();
                    self.print(value);
                }
                OpCode::Pop => {
                    self.pop();
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
                    let a = self.peek(0);
                    let b = self.peek(1);
                    match (a, b) {
                        (Value::StrPtr(_), Value::StrPtr(_)) => {
                            self.string_concat()?;
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
                OpCode::DefineGlobal(name_ptr) => {
                    let value = self.pop();
                    self.globals.insert(name_ptr, value);
                }
                OpCode::GetGlobal(name_ptr) => {
                    if !self.globals.contains_key(&name_ptr) {
                        return Err(InterpreterError::NameError(
                            self.current_line(),
                            format!("Undefined variable {}", self.get_string(name_ptr)),
                        ));
                    } else {
                        let value = self.globals[&name_ptr];
                        self.push(value);
                    }
                }
                OpCode::SetGlobal(name_ptr) => {
                    if !self.globals.contains_key(&name_ptr) {
                        return Err(InterpreterError::NameError(
                            self.current_line(),
                            format!("Undefined variable {}", self.get_string(name_ptr)),
                        ));
                    } else {
                        let value = *self.peek(0);
                        self.globals.insert(name_ptr, value);
                    }
                }
                OpCode::GetLocal(slot) => {
                    self.push(self.stack[slot]);
                }
                OpCode::SetLocal(slot) => {
                    self.stack[slot] = self.peek(0).clone();
                }
                OpCode::Jump(offset) => {
                    self.ip += offset;
                }
                OpCode::JumpIfFalse(offset) => {
                    if !Self::lox_bool_coercion(*self.peek(0)) {
                        self.ip += offset;
                    }
                }
                OpCode::Loop(offset) => {
                    self.ip -= offset;
                }
            }
        }
    }
}
