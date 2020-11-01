use super::chunk::*;
use super::value::{
    BoundMethod, Class, Closure, FromValue, Function, Instance, Object, ToValue, Value,
};
use std::collections::HashMap;
use std::mem::swap;

pub enum InterpreterError {
    TypeError(usize, String),
    NameError(usize, String),
    FunctionError(usize, String),
}

impl InterpreterError {
    pub fn to_string(&self) -> String {
        match self {
            InterpreterError::TypeError(line, msg)
            | InterpreterError::NameError(line, msg)
            | InterpreterError::FunctionError(line, msg) => format!("{}: {}", line, msg),
        }
    }
}

pub enum GCMark {
    Started,
    Complete,
}

#[derive(Clone, Copy)]
pub struct CallFrame {
    closure_pointer: u64,
    ip: usize,
    stack_pointer: usize,
}

pub struct VirtualMemory {
    pub heap: HashMap<u64, Object>,
    pub next_addr: u64,
    pub allocations: u64,
    pub max_allocations: u64,
}

impl VirtualMemory {
    pub fn new() -> VirtualMemory {
        let max_allocations = if cfg!(test_gc) { 5 } else { 500 };
        VirtualMemory {
            heap: HashMap::new(),
            next_addr: 0,
            allocations: 0,
            max_allocations,
        }
    }

    #[inline]
    pub fn next_addr_inner(&mut self) -> Option<u64> {
        while self.next_addr < u64::MAX {
            if !self.heap.contains_key(&self.next_addr) {
                let addr = self.next_addr;
                self.next_addr += 1;
                return Some(addr);
            }
            self.next_addr += 1;
        }

        None
    }

    pub fn next_addr(&mut self) -> u64 {
        if let Some(addr) = self.next_addr_inner() {
            addr
        } else {
            self.next_addr = 0;
            if let Some(addr) = self.next_addr_inner() {
                addr
            } else {
                panic!("Out of memory!");
            }
        }
    }

    #[inline]
    pub fn add_to_heap(&mut self, object: Object) -> u64 {
        self.allocations += 1;
        let new_address = self.next_addr();
        self.heap.insert(new_address, object);
        new_address
    }

    #[inline]
    pub fn remove_from_heap(&mut self, addr: u64) {
        self.heap.remove(&addr);
    }

    #[inline]
    pub fn deref(&self, ptr: u64) -> &Object {
        &self.heap[&ptr]
    }

    #[inline]
    pub fn deref_mut(&mut self, ptr: u64) -> &mut Object {
        self.heap.get_mut(&ptr).unwrap()
    }

    #[inline]
    fn closure_deref(&self, closure_p: u64) -> &Closure {
        self.heap[&closure_p].as_closure()
    }

    #[inline]
    fn fun_deref(&self, fun_p: u64) -> &Function {
        self.heap[&fun_p].as_fun()
    }

    #[inline]
    fn class_deref(&self, class_p: u64) -> &Class {
        self.heap[&class_p].as_class()
    }

    #[inline]
    fn value_deref(&self, value_ptr: u64) -> Value {
        self.heap[&value_ptr].as_value()
    }

    #[inline]
    fn write(&mut self, addr: u64, object: Object) {
        self.heap.insert(addr, object);
    }

    #[inline]
    fn function_deref(&self, fp: u64) -> &Function {
        self.heap[&fp].as_function()
    }

    #[inline]
    fn string_deref(&self, str_ptr: u64) -> &String {
        self.heap[&str_ptr].as_string()
    }
}

pub struct VM {
    stack: Vec<Value>,
    virtual_memory: Option<VirtualMemory>,
    globals: HashMap<String, Value>,
    //Never holds the active frame
    call_frames: Vec<CallFrame>,
    open_upvalues: Vec<(usize, usize, u64)>, //Nope, linear search.
}

impl VM {
    pub fn new() -> VM {
        VM {
            stack: vec![],
            virtual_memory: Some(VirtualMemory::new()),
            globals: HashMap::new(),
            call_frames: vec![],
            open_upvalues: vec![],
        }
    }

    pub fn take_virtual_memory(&mut self) -> VirtualMemory {
        let mut spare = None;
        swap(&mut spare, &mut self.virtual_memory);
        spare.unwrap()
    }

    pub fn give_virtual_memory(&mut self, virtual_memory: VirtualMemory) {
        self.virtual_memory = Some(virtual_memory);
    }

    pub fn interpret(
        &mut self,
        main: Function,
        virtual_memory: VirtualMemory,
    ) -> Result<(), InterpreterError> {
        self.virtual_memory = Some(virtual_memory);

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

    fn mark_object_started(gc_marks: &mut HashMap<u64, GCMark>, ptr: u64) -> bool {
        if !gc_marks.contains_key(&ptr) {
            gc_marks.insert(ptr, GCMark::Started);
            true
        } else {
            false
        }
    }

    fn mark_stack(&mut self, gc_marks: &mut HashMap<u64, GCMark>) {
        for value in self.stack.iter() {
            if let Value::Object(ptr) = value {
                Self::mark_object_started(gc_marks, *ptr);
            }
        }
    }

    fn mark_globals(&self, gc_marks: &mut HashMap<u64, GCMark>) {
        for val in self.globals.values() {
            if let Value::Object(ptr) = val {
                Self::mark_object_started(gc_marks, *ptr);
            }
        }
    }

    fn mark_callframes(&mut self, current_frame: &CallFrame, gc_marks: &mut HashMap<u64, GCMark>) {
        Self::mark_object_started(gc_marks, current_frame.closure_pointer);

        for frame in self.call_frames.iter() {
            Self::mark_object_started(gc_marks, frame.closure_pointer);
        }
    }

    #[inline]
    fn add_to_worklist(gc_marks: &mut HashMap<u64, GCMark>, worklist: &mut Vec<u64>, ptr: u64) {
        if Self::mark_object_started(gc_marks, ptr) {
            worklist.push(ptr);
        }
    }

    fn mark_object(&self, gc_marks: &mut HashMap<u64, GCMark>, worklist: &mut Vec<u64>, ptr: u64) {
        let object = self.heap().deref(ptr);
        match object {
            Object::Closure(closure) => {
                Self::add_to_worklist(gc_marks, worklist, closure.function_pointer);
                for closed_ptr in closure.closed_values.iter() {
                    Self::add_to_worklist(gc_marks, worklist, *closed_ptr);
                }
            }
            Object::Value(val) => {
                if let Value::Object(obj_ptr) = val {
                    Self::add_to_worklist(gc_marks, worklist, *obj_ptr);
                }
            }
            Object::Function(fun) => {
                for value in fun.chunk.constants.iter() {
                    if let Value::Object(obj_ptr) = value {
                        Self::add_to_worklist(gc_marks, worklist, *obj_ptr)
                    }
                }
            }
            Object::Instance(instance) => {
                Self::add_to_worklist(gc_marks, worklist, instance.class_ptr);
                for value in instance.fields.values() {
                    if let Value::Object(obj_ptr) = value {
                        Self::add_to_worklist(gc_marks, worklist, *obj_ptr);
                    }
                }
            }
            Object::Class(class) => {
                for closure_ptr in class.methods.values() {
                    Self::add_to_worklist(gc_marks, worklist, *closure_ptr);
                }
            }
            Object::BoundMethod(bound_method) => {
                if let Value::Object(ptr) = bound_method.receiver {
                    Self::add_to_worklist(gc_marks, worklist, ptr);
                }
                Self::add_to_worklist(gc_marks, worklist, bound_method.closure_ptr);
            }
            _ => {}
        }
    }

    fn sweep(&mut self, gc_marks: &HashMap<u64, GCMark>) {
        let mut to_remove: Vec<u64> = vec![];
        for ptr in self.heap().heap.keys() {
            if !gc_marks.contains_key(ptr) {
                to_remove.push(*ptr);
            }
        }
        for ptr in to_remove.iter() {
            if cfg!(test_gc) {
                println!("Removing {}", self.heap().deref(*ptr));
            }

            self.heap_mut().remove_from_heap(*ptr);
        }
    }

    fn collect_garbage(&mut self, current_frame: &CallFrame) {
        let mut gc_marks: HashMap<u64, GCMark> = HashMap::new();

        self.mark_stack(&mut gc_marks);
        self.mark_globals(&mut gc_marks);
        self.mark_callframes(current_frame, &mut gc_marks);

        let mut worklist: Vec<u64> = gc_marks.iter().map(|(k, _)| *k).collect();

        while worklist.len() > 0 {
            let ptr = worklist.pop().unwrap();
            if let GCMark::Started = gc_marks[&ptr] {
                self.mark_object(&mut gc_marks, &mut worklist, ptr);
                gc_marks.insert(ptr, GCMark::Complete);
            }
        }

        self.sweep(&gc_marks);

        self.heap_mut().allocations = 0;
    }

    fn should_run_gc(&self) -> bool {
        if self.heap().allocations > self.heap().max_allocations {
            true
        } else {
            false
        }
    }

    #[inline]
    fn heap_mut(&mut self) -> &mut VirtualMemory {
        self.virtual_memory.as_mut().unwrap()
    }

    #[inline]
    fn heap(&self) -> &VirtualMemory {
        &self.virtual_memory.as_ref().unwrap()
    }

    #[inline]
    fn get_closed_value(&self, frame: &CallFrame, index: usize) -> Value {
        let closure = self.heap().closure_deref(frame.closure_pointer);
        let value_ptr = closure.closed_values[index];
        let obj = self.heap().deref(value_ptr);
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
        let closure = self.heap().closure_deref(frame.closure_pointer);
        let value_ptr = closure.closed_values[index];
        let obj = self.heap().deref(value_ptr).clone(); //Only expensive in the case where we'll panic anyway...

        match obj {
            Object::OpenUpvalue(call_frame_idx, slot_idx) => {
                let closure_frame = self.call_frames[call_frame_idx];
                self.write_stack(&closure_frame, slot_idx, value);
            }
            Object::Value(_) => {
                self.heap_mut().write(value_ptr, Object::Value(value));
            }
            _ => {
                panic!("Attempt to write to closed value which is not an OpenUpvalue or Value type")
            }
        }
    }

    #[inline]
    fn chunk(&self, closure_p: u64) -> &Chunk {
        let fp = self.heap().closure_deref(closure_p).function_pointer;
        &self.heap().function_deref(fp).chunk
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

    fn deref_str_value(&self, value: Value) -> Result<&String, InterpreterError> {
        if let Value::Object(ptr) = value {
            if let Object::String(s) = self.heap().deref(ptr) {
                return Ok(s);
            }
        }

        Err(InterpreterError::TypeError(
            0,
            String::from("Expected two strings for '+' operator"),
        ))
    }

    fn string_concat(&mut self) -> Result<(), InterpreterError> {
        let b = self.pop();
        let a = self.pop();
        let s_a = self.deref_str_value(a)?;
        let s_b = self.deref_str_value(b)?;

        let s_c = format!("{}{}", s_a, s_b);
        let str_ptr = self.heap_mut().add_to_heap(Object::String(s_c));
        self.stack.push(Value::Object(str_ptr));
        Ok(())
    }

    fn values_equal(&self, a: Value, b: Value) -> bool {
        match (a, b) {
            (Value::Boolean(ba), Value::Boolean(bb)) => ba == bb,
            (Value::Number(na), Value::Number(nb)) => na == nb,
            (Value::Nil, Value::Nil) => true,
            (Value::Object(p_a), Value::Object(p_b)) => {
                let v_a = self.heap().deref(p_a);
                let v_b = self.heap().deref(p_b);
                match (v_a, v_b) {
                    (Object::String(s1), Object::String(s2)) => s1 == s2,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn print(&self, value: Value) {
        match value {
            Value::Object(p) => println!("{}", self.heap().deref(p)),
            _ => println!("{}", value),
        }
    }

    fn peek(&self, look_back: usize) -> &Value {
        &self.stack[self.stack.len() - 1 - look_back]
    }

    #[inline]
    fn add_to_heap(&mut self, object: Object) -> u64 {
        self.heap_mut().add_to_heap(object)
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
        let fun_def = self.heap().fun_deref(closure.function_pointer);

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

        let stack_pointer = self.stack.len() - (num_args + 1); // +1 for "this"
        let new_frame = CallFrame {
            closure_pointer: closure_p,
            ip: 0,
            stack_pointer,
        };
        Ok((*frame, new_frame))
    }

    fn search_captured_upvalue(&self, call_frame_idx: usize, slot: usize) -> Option<u64> {
        if let Some((_cf, _s, ptr)) = self
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
            let parent_closure = self.heap().closure_deref(frame.closure_pointer);
            parent_closure.closed_values[upvalue.index]
        }
    }

    fn run(&mut self) -> Result<(), InterpreterError> {
        let mut frame = self.call_frames.pop().unwrap();
        loop {
            if self.should_run_gc() {
                self.collect_garbage(&frame);
            }

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
                        self.heap_mut().write(*ptr, Object::Value(value));
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
                        (Value::Object(_), Value::Object(_)) => {
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
                OpCode::DefineGlobal(string_idx) => {
                    let name_ptr = u64::as_val_or_panic(self.read_constant(&frame, string_idx));
                    let name = self.heap().string_deref(name_ptr).clone();
                    let value = self.pop();
                    self.globals.insert(name, value);
                }
                OpCode::GetGlobal(string_idx) => {
                    let name_ptr = u64::as_val_or_panic(self.read_constant(&frame, string_idx));
                    let name = self.heap().string_deref(name_ptr);
                    if !self.globals.contains_key(name) {
                        return Err(InterpreterError::NameError(
                            self.current_line(&frame),
                            format!("Undefined variable {}", name),
                        ));
                    } else {
                        let value = self.globals[name];
                        self.push(value);
                    }
                }
                OpCode::SetGlobal(string_idx) => {
                    let name_ptr = u64::as_val_or_panic(self.read_constant(&frame, string_idx));
                    let name = self.heap().string_deref(name_ptr).clone();
                    if !self.globals.contains_key(&name) {
                        return Err(InterpreterError::NameError(
                            self.current_line(&frame),
                            format!("Undefined variable {}", name),
                        ));
                    } else {
                        let value = *self.peek(0);
                        self.globals.insert(name, value);
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
                    let obj_ptr = if let Value::Object(obj_ptr) = self.peek(num_args + 1) {
                        *obj_ptr
                    } else {
                        return Err(InterpreterError::FunctionError(
                            line,
                            String::from("Attempt to call a value which is not a function"),
                        ));
                    };
                    let obj = self.heap().deref(obj_ptr);

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
                        Object::Class(_) => {
                            let obj_instance = Object::Instance(Instance {
                                class_ptr: obj_ptr,
                                fields: HashMap::new(),
                            });
                            let addr = self.add_to_heap(obj_instance);
                            self.pop(); //This
                            self.push(Value::Object(addr));
                        }
                        Object::BoundMethod(bound_method) => {
                            let closure_ptr = bound_method.closure_ptr;
                            let closure = self.heap().closure_deref(closure_ptr);
                            let receiver = bound_method.receiver; //Copy here to drop the ref to bound_method
                            let (old_frame, new_frame) =
                                self.call_lox_function(&frame, closure, closure_ptr, num_args)?;
                            self.call_frames.push(old_frame);
                            frame = new_frame;
                            self.write_stack(&frame, 0, receiver);
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
                    self.heap_mut().write(ptr, Object::Value(value));
                }
                OpCode::Class(const_idx) => {
                    let value = self.read_constant(&frame, const_idx);
                    let ptr = u64::as_val_or_panic(value);
                    let name = self.heap().string_deref(ptr).clone();
                    let new_class = Object::Class(Class {
                        name,
                        methods: HashMap::new(),
                    });
                    let addr = self.add_to_heap(new_class);
                    self.push(Value::Object(addr));
                }
                OpCode::GetProperty(const_idx) => {
                    let line = self.current_line(&frame);
                    let name_ptr = u64::as_val_or_panic(self.read_constant(&frame, const_idx));
                    let name = self.heap().string_deref(name_ptr).clone(); //Can we eliminate this clone?

                    let instance_value = self.pop();
                    let instance_ptr = u64::as_val_or_panic(instance_value);
                    let object = self.heap().deref(instance_ptr);
                    if let Object::Instance(instance) = object {
                        let field_val = instance.fields.get(&name).copied();
                        if let Some(value) = field_val {
                            //Read the field
                            self.push(value);
                        } else {
                            //check if there's a method
                            let class = self.heap().class_deref(instance.class_ptr);
                            let closure_ptr = class.methods.get(&name).copied();
                            if let Some(closure_ptr) = closure_ptr {
                                let bound_method = Object::BoundMethod(BoundMethod {
                                    receiver: Value::Object(instance_ptr),
                                    closure_ptr,
                                });
                                let addr = self.add_to_heap(bound_method);
                                self.push(Value::Object(addr));
                            } else {
                                return Err(InterpreterError::NameError(
                                    line,
                                    format!("Undefined property {}", name),
                                ));
                            }
                        };
                    } else {
                        return Err(InterpreterError::TypeError(
                            line,
                            format!("Attempted to access field {}, but target was not an instance of an object", name),
                        ));
                    }
                }
                OpCode::SetProperty(const_idx) => {
                    let line = self.current_line(&frame);
                    let name_ptr = u64::as_val_or_panic(self.read_constant(&frame, const_idx));
                    let name = self.heap().string_deref(name_ptr).clone(); //Can we eliminate this clone?

                    let value_set = self.pop();

                    let instance_value = self.pop();
                    let instance_ptr = u64::as_val_or_panic(instance_value);
                    let object = self.heap_mut().deref_mut(instance_ptr);
                    if let Object::Instance(instance) = object {
                        instance.fields.insert(name, value_set);
                        self.push(value_set);
                    } else {
                        return Err(InterpreterError::TypeError(
                            line,
                            format!("Attempted to access field {}, but target was not an instance of an object", name),
                        ));
                    }
                }
                OpCode::Method(const_idx) => {
                    let string_ptr = u64::as_val_or_panic(self.read_constant(&frame, const_idx));
                    let method_name = self.heap().string_deref(string_ptr).clone();

                    let method_ptr = u64::as_val_or_panic(self.pop());

                    let class_ptr = u64::as_val_or_panic(*self.peek(0));
                    let class_obj = self.heap_mut().deref_mut(class_ptr);
                    if let Object::Class(class) = class_obj {
                        class.methods.insert(method_name, method_ptr);
                    } else {
                        panic!("Expected class object");
                    }
                }
                OpCode::ThisPlaceholder => {
                    self.push(Value::Nil);
                }
            }
        }
    }
}
