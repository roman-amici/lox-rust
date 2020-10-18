use super::chunk::*;
use super::value::{Closure, FromValue, Function, Object, ToValue, Value};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub enum InterpreterError {
    TypeError(usize, String),
    NameError(usize, String),
    FunctionError(usize, String),
    MemoryError(usize, String),
}

impl InterpreterError {
    pub fn to_string(&self) -> String {
        match self {
            InterpreterError::TypeError(line, msg)
            | InterpreterError::NameError(line, msg)
            | InterpreterError::FunctionError(line, msg)
            | InterpreterError::MemoryError(line, msg) => format!("{}: {}", line, msg),
        }
    }
}

#[derive(Clone, Copy)]
pub struct CallFrame {
    closure_pointer: u64,
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
    open_upvalues: Vec<(usize, usize, u64)>, //Nope, linear search.
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
            open_upvalues: vec![],
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
        main: Function,
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
        let closure_p = self.add_to_heap(Object::Closure(Closure {
            function_pointer: fp,
            closed_values: vec![],
        }));
        self.call_frames.push(CallFrame {
            closure_pointer: closure_p,
            ip: 0,
            stack_pointer: 0,
        }); //Will be immediately popped when run is called.
        self.run()
    }

    #[inline]
    fn deref(&self, ptr: u64, line: usize) -> Result<&Object, InterpreterError> {
        if self.heap.contains_key(&ptr) {
            Ok(&self.heap[&ptr])
        } else {
            Err(InterpreterError::MemoryError(
                line,
                String::from("Invalid pointer dereference"),
            ))
        }
    }

    #[inline]
    fn deref_internal(&self, ptr: u64) -> &Object {
        &self.heap[&ptr]
    }

    #[inline]
    fn closure_deref(&self, closure_p: u64) -> &Closure {
        self.heap[&closure_p].as_closure()
    }

    #[inline]
    fn value_deref(&self, value_ptr: u64) -> Value {
        self.heap[&value_ptr].as_value()
    }

    #[inline]
    fn get_closed_value(&self, frame: &CallFrame, index: usize) -> Value {
        let closure = self.closure_deref(frame.closure_pointer);
        let value_ptr = closure.closed_values[index];
        let obj = self.deref_internal(value_ptr);
        match obj {
            Object::OpenUpvalue(call_frame_idx, slot_idx) => {
                let closure_frame = self.call_frames[*call_frame_idx];
                self.read_stack(&closure_frame, *slot_idx)
            }
            Object::Value(value) => *value,
            _ => panic!("Attempt to get closed value which is not an OpenUpvalue or Value type"),
        }
    }

    #[inline]
    fn set_closed_value(&mut self, frame: &CallFrame, index: usize, value: Value) {
        let closure = self.closure_deref(frame.closure_pointer);
        let value_ptr = closure.closed_values[index];
        let obj = self.deref_internal(value_ptr).clone(); //Only expensive in the case where we'll panic anyway...

        match obj {
            Object::OpenUpvalue(call_frame_idx, slot_idx) => {
                let closure_frame = self.call_frames[call_frame_idx];
                self.write_stack(&closure_frame, slot_idx, value);
            }
            Object::Value(_) => {
                self.heap.insert(value_ptr, Object::Value(value));
            }
            _ => {
                panic!("Attempt to write to closed value which is not an OpenUpvalue or Value type")
            }
        }
    }

    #[inline]
    fn function_deref(&self, fp: u64) -> &Function {
        self.heap[&fp].as_function()
    }

    #[inline]
    fn chunk(&self, closure_p: u64) -> &Chunk {
        let fp = self.closure_deref(closure_p).function_pointer;
        &self.function_deref(fp).chunk
    }

    #[inline]
    fn code(&self, closure_p: u64) -> &Vec<OpCode> {
        &self.chunk(closure_p).code
    }

    #[inline]
    fn consume(&self, frame: &mut CallFrame) -> OpCode {
        let code = self.code(frame.closure_pointer);
        if frame.ip < code.len() {
            let op = code[frame.ip];
            frame.ip += 1;
            op
        } else {
            OpCode::EOF
        }
    }

    fn read_constant(&self, frame: &CallFrame, address: usize) -> Value {
        self.chunk(frame.closure_pointer).constants[address].clone()
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
        self.chunk(frame.closure_pointer).line_numbers[frame.ip]
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
            Value::Object(p) => println!("{}", self.follow(p)),
            Value::StrPtr(p) => println!("{}", self.get_string(p)),
            _ => println!("{}", value),
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

    fn write_to_heap(&mut self, addr: u64, object: Object) {
        self.heap.insert(addr, object);
    }

    fn read_stack(&self, frame: &CallFrame, offset: usize) -> Value {
        self.stack[frame.stack_pointer + offset]
    }

    fn write_stack(&mut self, frame: &CallFrame, offset: usize, value: Value) {
        self.stack[frame.stack_pointer + offset] = value;
    }

    fn call_lox_function<'a>(
        &'a self,
        frame: &CallFrame,
        closure: &Closure,
        closure_p: u64,
        num_args: usize,
    ) -> Result<(CallFrame, CallFrame), InterpreterError> {
        let line = self.current_line(&frame);
        let obj = self.deref(closure.function_pointer, line)?;

        let fun_def = if let Object::Function(fun_def) = obj {
            fun_def
        } else {
            panic!("Expected a function object");
        };

        if fun_def.arity != num_args {
            return Err(InterpreterError::FunctionError(
                line,
                format!("Expected {} arguments but got {}", num_args, fun_def.arity),
            ));
        }

        if self.call_frames.len() > 256 {
            return Err(InterpreterError::FunctionError(
                line,
                String::from("Stack overflow"),
            ));
        }

        let stack_pointer = self.stack.len() - num_args;
        let new_frame = CallFrame {
            closure_pointer: closure_p,
            ip: 0,
            stack_pointer,
        };
        Ok((*frame, new_frame))
    }

    fn search_captured_upvalue(&self, call_frame_idx: usize, slot: usize) -> Option<u64> {
        if let Some((cf, s, ptr)) = self
            .open_upvalues
            .iter()
            .find(|(cf, s, _)| *cf == call_frame_idx && *s == slot)
        {
            Some(*ptr)
        } else {
            None
        }
    }

    fn remove_open_upvalue(&mut self, call_frame_idx: usize, slot: usize) -> u64 {
        if let Some(idx) = self
            .open_upvalues
            .iter()
            .position(|(cf, s, _)| *cf == call_frame_idx && *s == slot)
        {
            let (_, _, ptr) = self.open_upvalues.remove(idx);
            ptr
        } else {
            panic!("Tried to remove open upvalue that doesn't exit!");
        }
    }

    fn capture_upvalue(&mut self, frame: &CallFrame, upvalue: Upvalue) -> u64 {
        if upvalue.is_local {
            let call_frame_idx = self.call_frames.len(); //Use n+1 since the current frame is not added yet.
            if let Some(ptr) = self.search_captured_upvalue(call_frame_idx, upvalue.index) {
                ptr
            } else {
                let ptr = self.add_to_heap(Object::OpenUpvalue(call_frame_idx, upvalue.index));
                self.open_upvalues
                    .push((call_frame_idx, upvalue.index, ptr));
                ptr
            }
        } else {
            //Already captured?
            let parent_closure = self.closure_deref(frame.closure_pointer);
            parent_closure.closed_values[upvalue.index]
        }
    }

    fn run(&mut self) -> Result<(), InterpreterError> {
        let mut frame = self.call_frames.pop().unwrap();
        loop {
            match self.consume(&mut frame) {
                OpCode::EOF => return Ok(()),
                OpCode::Return => {
                    let result = self.pop();
                    if self.call_frames.len() == 0 {
                        return Ok(());
                    }

                    let mut to_open_upvalues: Vec<(usize, usize, u64)> = vec![];
                    let mut to_remove: Vec<(usize, usize, u64)> = vec![];

                    let call_frame_idx = self.call_frames.len();
                    for (cf, s, ptr) in self.open_upvalues.iter() {
                        if *cf == call_frame_idx {
                            to_remove.push((*cf, *s, *ptr));
                        } else {
                            to_open_upvalues.push((*cf, *s, *ptr));
                        }
                    }

                    for (_, s, ptr) in to_remove.iter() {
                        let value = self.read_stack(&frame, *s);
                        self.write_to_heap(*ptr, Object::Value(value));
                    }

                    self.open_upvalues = to_open_upvalues;

                    //Pop the function values off the stack.
                    while self.stack.len() > frame.stack_pointer {
                        self.pop();
                    }
                    self.pop(); //And the function address

                    self.push(result);
                    frame = self.call_frames.pop().unwrap();
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
                OpCode::Call(num_args) => {
                    let line = self.current_line(&frame);
                    let obj_ptr = if let Value::Object(obj_ptr) = self.peek(num_args) {
                        *obj_ptr
                    } else {
                        return Err(InterpreterError::FunctionError(
                            line,
                            String::from("Attempt to call a value which is not a function"),
                        ));
                    };
                    let obj = self.deref(obj_ptr, line)?;

                    match obj {
                        Object::NativeFunction(_, body) => {
                            let body = *body;
                            let mut native_call_stack: Vec<Value> = vec![];
                            for _ in 0..num_args {
                                let value = self.pop();
                                native_call_stack.push(value);
                            }
                            let result = body(native_call_stack)?;
                            self.push(result);
                        }
                        Object::Closure(closure) => {
                            //Todo: upvalues
                            let (old_frame, new_frame) =
                                self.call_lox_function(&frame, &closure, obj_ptr, num_args)?;
                            self.call_frames.push(old_frame);
                            frame = new_frame;
                        }
                        _ => {
                            println!("{}", obj);
                            return Err(InterpreterError::FunctionError(
                                line,
                                String::from("Attempted to call an object that's not callable"),
                            ));
                        }
                    }
                }
                OpCode::Closure(idx, num_upvalues) => {
                    if let Value::Object(function_pointer) = self.read_constant(&frame, idx) {
                        let mut closed_values: Vec<u64> = vec![];
                        for _i in 0..num_upvalues {
                            if let OpCode::Upvalue(upvalue) = self.consume(&mut frame) {
                                closed_values.push(self.capture_upvalue(&frame, upvalue));
                            } else {
                                panic!("Expected upvalue op");
                            }
                        }
                        let closure_addr = self.add_to_heap(Object::Closure(Closure {
                            function_pointer,
                            closed_values,
                        }));
                        self.push(Value::Object(closure_addr));
                    } else {
                        panic!("Expected closure object");
                    }
                }
                OpCode::GetUpValue(value_index) => {
                    let value = self.get_closed_value(&frame, value_index);
                    self.push(value);
                }
                OpCode::SetUpValue(value_index) => {
                    let value = *self.peek(0);
                    self.set_closed_value(&frame, value_index, value);
                }
                OpCode::Upvalue(_) => {
                    panic!("Upvalue instruction should be handled by closure instruction")
                }
                OpCode::CloseUpvalue => {
                    let value = self.pop();
                    let call_frame_idx = self.call_frames.len();
                    let slot = self.stack.len() - frame.stack_pointer;

                    let ptr = self.remove_open_upvalue(call_frame_idx, slot);
                    self.write_to_heap(ptr, Object::Value(value));
                }
            }
        }
    }
}
