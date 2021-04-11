use super::chunk::Chunk;
use super::interpreter::InterpreterError;
use std::collections::HashMap;
use std::fmt;
use std::fmt::{Display, Formatter};

pub type LoxPtr = usize;

#[derive(Debug, Copy, Clone)]
pub enum Value {
    Number(f64),
    Boolean(bool),
    Object(LoxPtr), //index
    Nil,
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{} : Number", n),
            Value::Boolean(b) => write!(f, "{} : Boolean", b),
            Value::Nil => write!(f, "nil : Nil"),
            Value::Object(ptr) => {
                write!(f, "{} : ObjectPtr", ptr)
            }
        }
    }
}

//Consider changing to a struct
#[derive(Clone)]
pub enum Object {
    Empty,
    String(String),
    Function(Function),
    NativeFunction(String, fn(Vec<Value>) -> Result<Value, InterpreterError>),
    Closure(Closure),          //Reference to a function object
    Value(Value),              //Box type
    OpenUpvalue(usize, usize), //call_frame, slot
    Class(Class),
    Instance(Instance),
    BoundMethod(BoundMethod),
}

impl Object {
    pub fn as_function(&self) -> &Function {
        if let Object::Function(f) = self {
            f
        } else {
            panic!("Deref object is not a function.");
        }
    }

    pub fn as_closure(&self) -> &Closure {
        if let Object::Closure(c) = self {
            c
        } else {
            panic!("Deref object is not a closure.");
        }
    }

    pub fn as_value(&self) -> Value {
        if let Object::Value(v) = self {
            *v
        } else {
            panic!("Deref object is not a value");
        }
    }

    pub fn as_string(&self) -> &String {
        if let Object::String(s) = self {
            s
        } else {
            panic!("Deref object is not a string");
        }
    }

    pub fn as_class(&self) -> &Class {
        if let Object::Class(class) = self {
            class
        } else {
            panic!("Deref object is not a class");
        }
    }

    pub fn as_class_mut(&mut self) -> &mut Class {
        if let Object::Class(class) = self {
            class
        } else {
            panic!("Deref object is not a class");
        }
    }

    pub fn as_fun(&self) -> &Function {
        if let Object::Function(fun) = self {
            fun
        } else {
            panic!("Derfe object is not a function")
        }
    }
}

impl Display for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Object::Empty => write!(f, "<Empty>"),
            Object::String(s) => write!(f, "{}", s),
            Object::Function(func) => write!(f, "{}", func.to_string()),
            Object::NativeFunction(name, _) => write!(f, "<Native {}>", name),
            Object::Closure(closure) => write!(f, "<Closure {}>", closure.function_pointer),
            Object::Value(val) => write!(f, "{}", val),
            Object::OpenUpvalue(call_frame, slot) => {
                write!(f, "< cf: {}, slot : {}>", call_frame, slot)
            }
            Object::Class(class) => write!(
                f,
                "<class {} |{} methods|>",
                class.name,
                class.methods.len()
            ),
            Object::Instance(_) => write!(f, "<Object>"),
            Object::BoundMethod(bound_method) => {
                write!(f, "<BoundMethod {}>", bound_method.receiver)
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum FnType {
    Function,
    Initializer,
    Script,
    Method,
}

#[derive(Clone)]
pub struct Function {
    pub fn_type: FnType,
    pub arity: usize,
    pub chunk: Chunk,
    pub name: String,
    pub upvalue_count: usize,
}

#[derive(Clone)]
pub struct Closure {
    pub function_pointer: LoxPtr,
    pub closed_values: Vec<LoxPtr>,
}

impl Function {
    pub fn new(name: String, arity: usize, fn_type: FnType) -> Function {
        Function {
            fn_type,
            name,
            arity,
            chunk: Chunk::new(),
            upvalue_count: 0,
        }
    }

    pub fn to_string(&self) -> String {
        format!("<fn {}(args[{}])>", self.name, self.arity)
    }
}

#[derive(Clone)]
pub struct Class {
    pub name: String,
    pub methods: HashMap<String, LoxPtr>,
}

#[derive(Clone)]
pub struct Instance {
    pub class_ptr: LoxPtr,
    pub fields: HashMap<String, Value>,
}

#[derive(Clone)]
pub struct BoundMethod {
    pub receiver: Value,
    pub closure_ptr: LoxPtr,
}

pub trait FromValue
where
    Self: Sized,
{
    fn as_val(val: Value, line: usize) -> Result<Self, InterpreterError>;
    fn as_val_or_panic(val: Value) -> Self;
}

pub trait FromValueRef {
    fn as_val_ref(val: &Value, line: usize) -> Result<&Self, InterpreterError>;
}

impl FromValue for f64 {
    fn as_val(val: Value, line: usize) -> Result<f64, InterpreterError> {
        match val {
            Value::Number(n) => Ok(n),
            _ => Err(InterpreterError::TypeError(
                line,
                String::from("Expected a number"),
            )),
        }
    }
    fn as_val_or_panic(val: Value) -> f64 {
        match val {
            Value::Number(n) => n,
            _ => panic!("Expected a number"),
        }
    }
}

impl FromValue for LoxPtr {
    fn as_val(val: Value, line: usize) -> Result<LoxPtr, InterpreterError> {
        match val {
            Value::Object(ptr) => Ok(ptr),
            _ => Err(InterpreterError::TypeError(
                line,
                String::from("Expected an object"),
            )),
        }
    }
    fn as_val_or_panic(val: Value) -> LoxPtr {
        match val {
            Value::Object(ptr) => ptr,
            _ => panic!("Expected an object"),
        }
    }
}

impl FromValue for bool {
    fn as_val(val: Value, line: usize) -> Result<bool, InterpreterError> {
        match val {
            Value::Boolean(b) => Ok(b),
            _ => Err(InterpreterError::TypeError(
                line,
                String::from("Expected a boolean"),
            )),
        }
    }
    fn as_val_or_panic(val: Value) -> bool {
        match val {
            Value::Boolean(b) => b,
            _ => panic!("Expected a boolean"),
        }
    }
}

pub trait ToValue
where
    Self: Sized,
{
    fn to_value(raw: Self) -> Value;
}

impl ToValue for f64 {
    fn to_value(raw: f64) -> Value {
        Value::Number(raw)
    }
}

impl ToValue for bool {
    fn to_value(raw: bool) -> Value {
        Value::Boolean(raw)
    }
}
