use super::interpreter::InterpreterError;

#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    Boolean(bool),
    Object(usize),
    Nil,
}

//Consider changing to a struct
#[derive(Debug, Clone)]
pub enum Object {
    String(String),
}

pub trait FromValue
where
    Self: Sized,
{
    fn as_val(val: Value, line: usize) -> Result<Self, InterpreterError>;
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
