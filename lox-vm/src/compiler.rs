use super::chunk::*;
use super::token::*;
use super::value::*;

use num_enum::TryFromPrimitive;
use std::convert::TryFrom;

pub enum CompilerError {
    SyntaxError(String, usize),
}

impl CompilerError {
    pub fn to_string(&self) -> String {
        match self {
            CompilerError::SyntaxError(message, line) => format!("{} : {}", line, message),
            _ => unimplemented!(),
        }
    }
}

#[derive(Copy, Clone)]
struct ParseRule {
    prefix: Option<fn(&mut Compiler, bool) -> Result<(), CompilerError>>,
    infix: Option<fn(&mut Compiler, bool) -> Result<(), CompilerError>>,
    precedence: Precedence,
}

pub struct Compiler {
    tokens: Vec<Token>,
    current: usize,
    chunk: Chunk,
    rules: Vec<ParseRule>,
    has_error: bool,
}

#[derive(PartialOrd, PartialEq, Ord, Eq, Copy, Clone, TryFromPrimitive)]
#[repr(usize)]
enum Precedence {
    //Allow Precedence to be used an index
    None = 0,
    Assignment,
    Or,
    And,
    Equality,
    Comparison,
    Term,
    Factor,
    Unary,
    Call,
    Primary,
}

impl Precedence {
    fn next(&self) -> Option<Precedence> {
        let as_num = (*self) as usize;
        if let Ok(precedence) = Precedence::try_from(as_num) {
            Some(precedence)
        } else {
            None
        }
    }
}

impl Compiler {
    pub fn new(tokens: Vec<Token>) -> Compiler {
        Compiler {
            tokens,
            current: 0,
            chunk: Chunk::new(),
            rules: Compiler::build_parse_rules(),
            has_error: false,
        }
    }

    fn build_parse_rules() -> Vec<ParseRule> {
        let start: usize = 0;
        let end = TokenType::EOF as usize + 1;

        let mut rules: Vec<ParseRule> = vec![];

        for i in start..end {
            let token_type: TokenType = TokenType::try_from(i).unwrap();
            match token_type {
                TokenType::LeftParen => rules.push(ParseRule {
                    prefix: Some(Compiler::grouping),
                    infix: None,
                    precedence: Precedence::None,
                }),
                TokenType::Minus => rules.push(ParseRule {
                    prefix: Some(Compiler::unary),
                    infix: Some(Compiler::binary),
                    precedence: Precedence::Term,
                }),
                TokenType::Plus => rules.push(ParseRule {
                    prefix: None,
                    infix: Some(Compiler::binary),
                    precedence: Precedence::Term,
                }),
                TokenType::Slash => rules.push(ParseRule {
                    prefix: None,
                    infix: Some(Compiler::binary),
                    precedence: Precedence::Factor,
                }),
                TokenType::Star => rules.push(ParseRule {
                    prefix: None,
                    infix: Some(Compiler::binary),
                    precedence: Precedence::Factor,
                }),
                TokenType::NumberToken => rules.push(ParseRule {
                    prefix: Some(Compiler::number),
                    infix: None,
                    precedence: Precedence::None,
                }),
                TokenType::False => rules.push(ParseRule {
                    prefix: Some(Compiler::literal),
                    infix: None,
                    precedence: Precedence::None,
                }),
                TokenType::True => rules.push(ParseRule {
                    prefix: Some(Compiler::literal),
                    infix: None,
                    precedence: Precedence::None,
                }),
                TokenType::Nil => rules.push(ParseRule {
                    prefix: Some(Compiler::literal),
                    infix: None,
                    precedence: Precedence::None,
                }),
                TokenType::Bang => rules.push(ParseRule {
                    prefix: Some(Compiler::unary),
                    infix: None,
                    precedence: Precedence::None,
                }),
                TokenType::EqualEqual => rules.push(ParseRule {
                    prefix: None,
                    infix: Some(Compiler::binary),
                    precedence: Precedence::Equality,
                }),
                TokenType::BangEqual => rules.push(ParseRule {
                    prefix: None,
                    infix: Some(Compiler::binary),
                    precedence: Precedence::Equality,
                }),
                TokenType::Greater => rules.push(ParseRule {
                    prefix: None,
                    infix: Some(Compiler::binary),
                    precedence: Precedence::Comparison,
                }),
                TokenType::GreaterEqual => rules.push(ParseRule {
                    prefix: None,
                    infix: Some(Compiler::binary),
                    precedence: Precedence::Comparison,
                }),
                TokenType::Less => rules.push(ParseRule {
                    prefix: None,
                    infix: Some(Compiler::binary),
                    precedence: Precedence::Comparison,
                }),
                TokenType::LessEqual => rules.push(ParseRule {
                    prefix: None,
                    infix: Some(Compiler::binary),
                    precedence: Precedence::Comparison,
                }),
                TokenType::StringToken => rules.push(ParseRule {
                    prefix: Some(Compiler::string),
                    infix: None,
                    precedence: Precedence::None,
                }),
                TokenType::Identifier => rules.push(ParseRule {
                    prefix: Some(Compiler::variable),
                    infix: None,
                    precedence: Precedence::None,
                }),
                _ => rules.push(ParseRule {
                    prefix: None,
                    infix: None,
                    precedence: Precedence::None,
                }),
            };
        }

        rules
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    pub fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len() || self.peek().token_type == TokenType::EOF
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    fn check_token(&mut self, token_type: TokenType) -> bool {
        if self.current < self.tokens.len() && self.peek().token_type == token_type {
            true
        } else {
            false
        }
    }

    fn match_token(&mut self, token_type: TokenType) -> bool {
        if self.check_token(token_type) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
            self.previous()
        } else {
            self.previous()
        }
    }

    fn try_consume(
        &mut self,
        token_type: TokenType,
        err_message: &str,
    ) -> Result<Token, CompilerError> {
        if self.is_at_end() {
            Err(CompilerError::SyntaxError(
                String::from(err_message),
                self.previous().line,
            ))
        } else {
            let token = self.peek().clone();
            if token.token_type == token_type {
                self.advance();
                Ok(token)
            } else {
                self.advance();
                Err(CompilerError::SyntaxError(
                    String::from(err_message),
                    token.line,
                ))
            }
        }
    }

    fn emit_constant(&mut self, value: Value, line: usize) -> Result<(), CompilerError> {
        let const_idx = self.chunk.add_constant(value);
        self.chunk.append_chunk(OpCode::Constant(const_idx), line);
        Ok(())
    }

    fn variable(&mut self, can_assign: bool) -> Result<(), CompilerError> {
        let (line, name) = {
            let token = self.previous();
            let name = token.literal.as_ref().unwrap().clone();
            (token.line, name)
        };
        let hash_value = self.chunk.add_string(name);

        if can_assign && self.match_token(TokenType::Equal) {
            self.expression()?;
            self.chunk.append_chunk(OpCode::SetGlobal(hash_value), line);
        } else {
            self.chunk.append_chunk(OpCode::GetGlobal(hash_value), line);
        }
        Ok(())
    }

    fn number(&mut self, can_assign: bool) -> Result<(), CompilerError> {
        let token = self.previous();
        assert_eq!(token.token_type, TokenType::NumberToken);

        let number: f64 = token.literal.as_ref().unwrap().parse().unwrap();
        let line = token.line;

        self.emit_constant(Value::Number(number), line)
    }

    fn literal(&mut self, can_assign: bool) -> Result<(), CompilerError> {
        let (token_type, line) = {
            let token = self.previous();
            (token.token_type, token.line)
        };
        match token_type {
            TokenType::False => self.chunk.append_chunk(OpCode::False, line),
            TokenType::True => self.chunk.append_chunk(OpCode::True, line),
            TokenType::Nil => self.chunk.append_chunk(OpCode::Nil, line),
            _ => {
                return Err(CompilerError::SyntaxError(
                    String::from("Expected literal"),
                    line,
                ))
            }
        }

        Ok(())
    }

    fn parse_precedence(&mut self, precedence: Precedence) -> Result<(), CompilerError> {
        let (token_type, line) = {
            let token = self.advance();
            (token.token_type, token.line)
        };
        if let Some(prefix_fn) = self.get_rule(token_type).prefix {
            let can_assign = precedence <= Precedence::Assignment;
            prefix_fn(self, can_assign)?; // Calls as a method
        } else {
            return Err(CompilerError::SyntaxError(
                String::from("Expected expression."),
                line,
            ));
        }

        while precedence <= self.get_rule(self.peek().token_type).precedence {
            let (token_type, line) = {
                let token = self.advance();
                (token.token_type, token.line)
            };
            if let Some(infix_fn) = self.get_rule(token_type).infix {
                let can_assign = precedence <= Precedence::Assignment;
                infix_fn(self, can_assign)?;
            } else {
                return Err(CompilerError::SyntaxError(
                    String::from("Expected expression."),
                    line,
                ));
            }
        }

        Ok(())
    }

    fn expression(&mut self) -> Result<(), CompilerError> {
        self.parse_precedence(Precedence::Assignment)
    }

    fn grouping(&mut self, can_assign: bool) -> Result<(), CompilerError> {
        self.expression()?;
        self.try_consume(TokenType::RightParen, "Expected ')' after expression")?;
        Ok(())
    }

    fn get_rule(&self, token_type: TokenType) -> &ParseRule {
        let rule_idx = token_type as usize;
        &self.rules[rule_idx]
    }

    fn binary(&mut self, can_assign: bool) -> Result<(), CompilerError> {
        let (token_type, line) = {
            let operator = self.previous();
            (operator.token_type, operator.line)
        };

        //Parse operators of higher precedence first
        let new_precedence = self.get_rule(token_type).precedence.next().unwrap();
        self.parse_precedence(new_precedence)?;

        //Deal with the token itself
        match token_type {
            TokenType::Plus => self.chunk.append_chunk(OpCode::Add, line),
            TokenType::Minus => self.chunk.append_chunk(OpCode::Subtract, line),
            TokenType::Star => self.chunk.append_chunk(OpCode::Multiply, line),
            TokenType::Slash => self.chunk.append_chunk(OpCode::Divide, line),
            TokenType::EqualEqual => self.chunk.append_chunk(OpCode::Equal, line),
            TokenType::BangEqual => {
                self.chunk.append_chunk(OpCode::Equal, line);
                self.chunk.append_chunk(OpCode::Not, line);
            }
            TokenType::Greater => self.chunk.append_chunk(OpCode::Greater, line),
            TokenType::GreaterEqual => {
                self.chunk.append_chunk(OpCode::Less, line);
                self.chunk.append_chunk(OpCode::Not, line);
            }
            TokenType::Less => self.chunk.append_chunk(OpCode::Less, line),
            TokenType::LessEqual => {
                self.chunk.append_chunk(OpCode::Greater, line);
                self.chunk.append_chunk(OpCode::Not, line);
            }
            _ => unimplemented!(),
        };

        Ok(())
    }

    fn string(&mut self, can_assign: bool) -> Result<(), CompilerError> {
        let (str_value, line) = {
            let token = self.previous();
            assert_eq!(token.token_type, TokenType::StringToken);
            let str_value = token.literal.as_ref().unwrap().clone();
            (str_value, token.line)
        };
        let hash_value = self.chunk.add_string(str_value);
        let const_idx = self.chunk.add_constant(Value::StrPtr(hash_value));
        self.chunk.append_chunk(OpCode::Constant(const_idx), line);
        Ok(())
    }

    fn unary(&mut self, can_assign: bool) -> Result<(), CompilerError> {
        let (token_type, line) = {
            let operator = self.previous();
            (operator.token_type, operator.line)
        };

        self.parse_precedence(Precedence::Unary)?;

        match token_type {
            TokenType::Minus => self.chunk.append_chunk(OpCode::Negate, line),
            TokenType::Bang => self.chunk.append_chunk(OpCode::Not, line),
            _ => unimplemented!(),
        }

        Ok(())
    }

    fn print_statement(&mut self) -> Result<(), CompilerError> {
        self.expression()?;
        let line = self
            .try_consume(TokenType::Semicolon, "Expected ';' after expression")?
            .line;

        self.chunk.append_chunk(OpCode::Print, line);

        Ok(())
    }

    fn expression_statement(&mut self) -> Result<(), CompilerError> {
        self.expression()?;
        let line = self
            .try_consume(TokenType::Semicolon, "Expected ';' after expression")?
            .line;

        self.chunk.append_chunk(OpCode::Pop, line);

        Ok(())
    }

    fn statement(&mut self) -> Result<(), CompilerError> {
        if self.match_token(TokenType::Print) {
            self.print_statement()
        } else {
            self.expression_statement()
        }
    }

    fn parse_variable(&mut self, error_msg: &str) -> Result<u64, CompilerError> {
        let token = self.try_consume(TokenType::Identifier, error_msg)?;
        let name = token.literal.unwrap().clone();

        Ok(self.chunk.add_string(name))
    }

    fn var_declaration(&mut self) -> Result<(), CompilerError> {
        let name_hash = self.parse_variable("Expected variable name.")?;
        let line = self.previous().line;

        if self.match_token(TokenType::Equal) {
            self.expression()?;
        } else {
            self.chunk.append_chunk(OpCode::Nil, line);
        }

        self.try_consume(
            TokenType::Semicolon,
            "Expected ';' after variable declaration",
        )?;

        self.chunk
            .append_chunk(OpCode::DefineGlobal(name_hash), line);
        Ok(())
    }

    fn declaration(&mut self) -> Result<(), CompilerError> {
        if self.match_token(TokenType::Var) {
            self.var_declaration()
        } else {
            self.statement()
        }
    }

    fn synchronize(&mut self) {
        while !self.is_at_end() {
            if self.previous().token_type == TokenType::Semicolon {
                return;
            } else {
                match self.peek().token_type {
                    TokenType::Class
                    | TokenType::Fun
                    | TokenType::Var
                    | TokenType::For
                    | TokenType::If
                    | TokenType::While
                    | TokenType::Print
                    | TokenType::Return => return,
                    _ => {
                        self.advance();
                    }
                }
            }
        }
    }

    pub fn compile(&mut self) -> Result<Chunk, ()> {
        let mut old_idx = self.current;
        while !self.is_at_end() {
            let result = self.declaration();
            if let Err(e) = result {
                self.has_error = true;
                println!("Compiler error: {}", e.to_string());
                self.synchronize();
            };

            if self.current == old_idx {
                println!("Error: Infinite loop");
                return Err(());
            }

            old_idx = self.current;
        }
        if self.has_error {
            Err(())
        } else {
            Ok(self.chunk.clone())
        }
    }
}
