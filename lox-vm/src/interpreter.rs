use super::chunk::*;
use super::value::{FromValue, FromValueRef, Function, Object, ToValue, Value};
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

#[derive(Clone, Copy)]
pub struct CallFrame {
    function_pointer: u64,
    ip: usize,
    stack_pointer: usize,
}

pub struct VM {
    stack: Vec<Value>,
    heap: HashMap<u64, Object>,
    next_addr: u64,
    globals: HashMap<u64, Value>,
    strings: HashMap<u64, String>,
    //Never holds the active frame
    call_frames: Vec<CallFrame>,
}

impl VM {
    pub fn new() -> VM {
        VM {
            stack: vec![],
            heap: HashMap::new(),
            next_addr: 0,
            globals: HashMap::new(),
            strings: HashMap::new(),
            call_frames: vec![],
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

    pub fn next_addr(&mut self) -> u64 {
        let addr = self.next_addr;
        self.next_addr += 1;
        addr
    }

    pub fn interpret(
        &mut self,
        mut main: Function,
        new_strings: Vec<String>,
        new_objects: Vec<(u64, Object)>,
    ) -> Result<(), InterpreterError> {
        for (addr, o) in new_objects {
            self.add_to_heap_addr(addr, o);
        }

        for s in new_strings {
            self.add_new_string(s);
        }

        let fp = self.add_to_heap(Object::Function(main));
        self.call_frames.push(CallFrame {
            function_pointer: fp,
            ip: 0,
            stack_pointer: 0,
        }); //Will be immediately popped when run is called.
        self.run()
    }

    #[inline]
    fn function_dref(&self, fp: u64) -> &Function {
        self.heap[&fp].as_function()
    }

    #[inline]
    fn chunk(&self, fp: u64) -> &Chunk {
        &self.function_dref(fp).chunk
    }

    #[inline]
    fn code(&self, fp: u64) -> &Vec<OpCode> {
        &self.chunk(fp).code
    }

    #[inline]
    fn consume(&self, frame: &mut CallFrame) -> OpCode {
        let code = self.code(frame.function_pointer);
        if frame.ip < code.len() {
            let op = code[frame.ip];
            frame.ip += 1;
            op
        } else {
            OpCode::EOF
        }
    }

    fn read_constant(&self, frame: &CallFrame, address: usize) -> Value {
        self.chunk(frame.function_pointer).constants[address].clone()
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
    fn current_line(&self, frame: &CallFrame) -> usize {
        //Since we've already advanced past it
        self.chunk(frame.function_pointer).line_numbers[frame.ip]
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
        frame: &CallFrame,
        bop: fn(T1, T2) -> R,
    ) -> Result<(), InterpreterError> {
        let line = self.current_line(frame);
        let b = T2::as_val(self.pop(), line)?;
        let a = T1::as_val(self.pop(), line)?;
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
            Value::Object(p) => println!("{}", self.follow(p)),
            Value::Nil => println!("nil : Nil"),
            Value::StrPtr(p) => println!("{}", self.get_string(p)),
        }
    }

    fn peek(&self, look_back: usize) -> &Value {
        &self.stack[self.stack.len() - 1 - look_back]
    }

    fn follow(&self, pointer: u64) -> &Object {
        &self.heap[&pointer]
    }

    fn add_to_heap_addr(&mut self, addr: u64, object: Object) {
        if self.heap.contains_key(&addr) {
            panic!("Heap collision for {}", addr);
        } else {
            self.heap.insert(addr, object);
        }
    }
    fn add_to_heap(&mut self, object: Object) -> u64 {
        let new_address = self.next_addr();
        self.heap.insert(new_address, object);
        new_address
    }

    fn read_stack(&self, frame: &CallFrame, offset: usize) -> Value {
        self.stack[frame.stack_pointer + offset]
    }

    fn write_stack(&mut self, frame: &CallFrame, offset: usize, value: Value) {
        self.stack[frame.stack_pointer + offset] = value;
    }

    fn run(&mut self) -> Result<(), InterpreterError> {
        let mut frame = self.call_frames.pop().unwrap();
        loop {
            match self.consume(&mut frame) {
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
                    let val = self.read_constant(&frame, address);
                    self.push(val);
                }
                OpCode::Negate => match self.pop() {
                    Value::Number(n) => self.push(Value::Number(-n)),
                    _ => {
                        return Err(InterpreterError::TypeError(
                            self.current_line(&frame),
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
                        _ => self.binary_op(&frame, |a: f64, b: f64| a + b)?,
                    };
                }
                OpCode::Subtract => {
                    self.binary_op(&frame, |a: f64, b: f64| a - b)?;
                }
                OpCode::Multiply => {
                    self.binary_op(&frame, |a: f64, b: f64| a * b)?;
                }
                OpCode::Divide => {
                    self.binary_op(&frame, |a: f64, b: f64| a / b)?;
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
                    self.binary_op(&frame, |a: f64, b: f64| a > b)?;
                }
                OpCode::Less => {
                    self.binary_op(&frame, |a: f64, b: f64| a < b)?;
                }
                OpCode::DefineGlobal(name_ptr) => {
                    let value = self.pop();
                    self.globals.insert(name_ptr, value);
                }
                OpCode::GetGlobal(name_ptr) => {
                    if !self.globals.contains_key(&name_ptr) {
                        return Err(InterpreterError::NameError(
                            self.current_line(&frame),
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
                            self.current_line(&frame),
                            format!("Undefined variable {}", self.get_string(name_ptr)),
                        ));
                    } else {
                        let value = *self.peek(0);
                        self.globals.insert(name_ptr, value);
                    }
                }
                OpCode::GetLocal(slot) => {
                    let value = self.read_stack(&frame, slot);
                    self.push(value);
                }
                OpCode::SetLocal(slot) => {
                    let value = self.peek(0).clone();
                    self.write_stack(&frame, slot, value);
                }
                OpCode::Jump(offset) => {
                    frame.ip += offset;
                }
                OpCode::JumpIfFalse(offset) => {
                    if !Self::lox_bool_coercion(*self.peek(0)) {
                        frame.ip += offset;
                    }
                }
                OpCode::Loop(offset) => {
                    frame.ip -= offset;
                }
            }
        }
    }
}
