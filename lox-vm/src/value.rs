use super::interpreter::InterpreterError;

#[derive(Debug, Copy, Clone)]
pub enum Value {
    Number(f64),
}

impl Value {
    #[inline]
    pub fn as_number(self) -> Result<f64, InterpreterError> {
        match self {
            Value::Number(n) => Ok(n),
            _ => Err(InterpreterError::RuntimeError),
        }
    }
}
