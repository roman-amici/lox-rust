use super::chunk::Chunk;
use super::interpreter::InterpreterError;

#[derive(Debug, Copy, Clone)]
pub enum Value {
    Number(f64),
    Boolean(bool),
    Object(usize),
    StrPtr(u64),
    Nil,
}

//Consider changing to a struct
pub enum Object {
    String(String),
    Function(Function),
}

impl Object {
    pub fn as_function(&self) -> &Function {
        if let Object::Function(f) = self {
            f
        } else {
            panic!();
        }
    }
}

#[derive(Clone, Copy)]
pub enum FnType {
    Function,
    Script,
}

#[derive(Clone)]
pub struct Function {
    pub fn_type: FnType,
    pub arity: usize,
    pub chunk: Chunk,
    pub name: String,
}

impl Function {
    pub fn new(name: String, arity: usize, fn_type: FnType) -> Function {
        Function {
            fn_type,
            name,
            arity,
            chunk: Chunk::new(),
        }
    }

    pub fn to_string(&self) -> String {
        format!("fn {}({})", self.name, self.arity)
    }
}

pub trait FromValue
where
    Self: Sized,
{
    fn as_val(val: Value, line: usize) -> Result<Self, InterpreterError>;
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
