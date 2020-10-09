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

struct Local {
    pub name: Token,
    pub depth: usize,
    pub initialized: bool,
}

pub struct Compiler {
    tokens: Vec<Token>,
    current: usize,
    chunk: Chunk,
    rules: Vec<ParseRule>,
    has_error: bool,
    locals: Vec<Local>,
    scope_depth: usize,
}

impl Compiler {
    pub fn new(tokens: Vec<Token>) -> Compiler {
        Compiler {
            tokens,
            current: 0,
            chunk: Chunk::new(),
            rules: Compiler::build_parse_rules(),
            has_error: false,
            locals: vec![],
            scope_depth: 0,
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
                TokenType::BangEqual => rules.push(ParseRule {
                    prefix: None,
                    infix: Some(Compiler::binary),
                    precedence: Precedence::Equality,
                }),
                TokenType::Equal => rules.push(ParseRule {
                    prefix: None,
                    infix: None,
                    precedence: Precedence::None,
                }),
                TokenType::EqualEqual => rules.push(ParseRule {
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
                TokenType::Identifier => rules.push(ParseRule {
                    prefix: Some(Compiler::variable),
                    infix: None,
                    precedence: Precedence::None,
                }),
                TokenType::StringToken => rules.push(ParseRule {
                    prefix: Some(Compiler::string),
                    infix: None,
                    precedence: Precedence::None,
                }),
                TokenType::NumberToken => rules.push(ParseRule {
                    prefix: Some(Compiler::number),
                    infix: None,
                    precedence: Precedence::None,
                }),
                TokenType::And => rules.push(ParseRule {
                    prefix: None,
                    infix: Some(Compiler::and),
                    precedence: Precedence::And,
                }),
                TokenType::Or => rules.push(ParseRule {
                    prefix: None,
                    infix: Some(Compiler::or),
                    precedence: Precedence::Or,
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

    fn resolve_local(
        &self,
        var_name: &String,
        line: usize,
    ) -> Result<Option<usize>, CompilerError> {
        if self.locals.len() == 0 {
            return Ok(None);
        }
        let mut idx = self.locals.len() - 1;
        for local in self.locals.iter().rev() {
            if local.name.lexeme == *var_name {
                if local.initialized {
                    return Ok(Some(idx));
                } else {
                    return Err(CompilerError::SyntaxError(
                        String::from("Can't read local variable in its own initializer."),
                        line,
                    ));
                }
            }
            idx -= 1;
        }

        Ok(None)
    }

    fn variable(&mut self, can_assign: bool) -> Result<(), CompilerError> {
        let (line, name) = {
            let token = self.previous();
            let name = token.literal.as_ref().unwrap().clone();
            (token.line, name)
        };

        let (set_op, get_op) = if let Some(id) = self.resolve_local(&name, line)? {
            (OpCode::SetLocal(id), OpCode::GetLocal(id))
        } else {
            let hash_value = self.chunk.add_string(name);
            (OpCode::SetGlobal(hash_value), OpCode::GetGlobal(hash_value))
        };

        if can_assign && self.match_token(TokenType::Equal) {
            self.expression()?;
            self.chunk.append_chunk(set_op, line);
        } else {
            self.chunk.append_chunk(get_op, line);
        }
        Ok(())
    }

    fn number(&mut self, _can_assign: bool) -> Result<(), CompilerError> {
        let token = self.previous();
        assert_eq!(token.token_type, TokenType::NumberToken);

        let number: f64 = token.literal.as_ref().unwrap().parse().unwrap();
        let line = token.line;

        self.emit_constant(Value::Number(number), line)
    }

    fn literal(&mut self, _can_assign: bool) -> Result<(), CompilerError> {
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
        };

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

    fn grouping(&mut self, _can_assign: bool) -> Result<(), CompilerError> {
        self.expression()?;
        self.try_consume(TokenType::RightParen, "Expected ')' after expression")?;
        Ok(())
    }

    fn get_rule(&self, token_type: TokenType) -> &ParseRule {
        let rule_idx = token_type as usize;
        &self.rules[rule_idx]
    }

    fn binary(&mut self, _can_assign: bool) -> Result<(), CompilerError> {
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
                self.chunk.append_chunk(OpCode::Not, line)
            }
            TokenType::Greater => self.chunk.append_chunk(OpCode::Greater, line),
            TokenType::GreaterEqual => {
                self.chunk.append_chunk(OpCode::Less, line);
                self.chunk.append_chunk(OpCode::Not, line)
            }
            TokenType::Less => self.chunk.append_chunk(OpCode::Less, line),
            TokenType::LessEqual => {
                self.chunk.append_chunk(OpCode::Greater, line);
                self.chunk.append_chunk(OpCode::Not, line)
            }
            _ => unimplemented!(),
        };

        Ok(())
    }

    fn string(&mut self, _can_assign: bool) -> Result<(), CompilerError> {
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

    fn unary(&mut self, _can_assign: bool) -> Result<(), CompilerError> {
        let (token_type, line) = {
            let operator = self.previous();
            (operator.token_type, operator.line)
        };

        self.parse_precedence(Precedence::Unary)?;

        match token_type {
            TokenType::Minus => self.chunk.append_chunk(OpCode::Negate, line),
            TokenType::Bang => self.chunk.append_chunk(OpCode::Not, line),
            _ => unimplemented!(),
        };

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

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.scope_depth -= 1;
        while {
            match self.locals.last() {
                Some(local) => local.depth > self.scope_depth, //Since we just decremented the depth
                None => false,
            }
        } {
            let local = self.locals.pop().unwrap();
            self.chunk.append_chunk(OpCode::Pop, local.name.line);
        }
    }

    fn block(&mut self) -> Result<(), CompilerError> {
        while !self.check_token(TokenType::RightBrace) && !self.check_token(TokenType::EOF) {
            self.declaration()?;
        }

        self.try_consume(TokenType::RightBrace, "Expected '}' after block.")?;
        Ok(())
    }

    fn statement(&mut self) -> Result<(), CompilerError> {
        if self.match_token(TokenType::Print) {
            self.print_statement()
        } else if self.match_token(TokenType::LeftBrace) {
            self.begin_scope();
            self.block()?;
            self.end_scope();
            Ok(())
        } else if self.match_token(TokenType::If) {
            self.if_statement()
        } else if self.match_token(TokenType::While) {
            self.while_statement()
        } else if self.match_token(TokenType::For) {
            self.for_statement()
        } else {
            self.expression_statement()
        }
    }

    fn parse_variable(&mut self, error_msg: &str) -> Result<u64, CompilerError> {
        let token = self.try_consume(TokenType::Identifier, error_msg)?;

        if self.scope_depth > 0 {
            self.locals.push(Local {
                name: token.clone(),
                depth: self.scope_depth,
                initialized: false,
            });
            //I think shadowing is fine, so we won't look for duplicate id's

            Ok(0) //Us a dummy address
        } else {
            let name = token.literal.unwrap().clone();
            Ok(self.chunk.add_string(name))
        }
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

        if self.scope_depth == 0 {
            //Only define globals at scope depth
            self.chunk
                .append_chunk(OpCode::DefineGlobal(name_hash), line);
        } else {
            self.locals.last_mut().unwrap().initialized = true;
        }
        Ok(())
    }

    fn declaration(&mut self) -> Result<(), CompilerError> {
        if self.match_token(TokenType::Var) {
            self.var_declaration()
        } else {
            self.statement()
        }
    }

    fn patch_jump(&mut self, instruction_idx: usize) {
        let offset = self.chunk.top() - instruction_idx;
        self.chunk.patch_jump(instruction_idx, offset);
    }

    fn if_statement(&mut self) -> Result<(), CompilerError> {
        self.try_consume(TokenType::LeftParen, "Expected '(' after 'if'.")?;
        self.expression()?;
        let line = self
            .try_consume(TokenType::RightParen, "Expected ')' after condition.")?
            .line;

        //Dummy value for jump which we will patch in later
        let if_jump = self.chunk.append_chunk(OpCode::JumpIfFalse(0), line);

        //We leave the predicate on the stack due to short circuiting (see 'and')
        self.chunk.append_chunk(OpCode::Pop, line);
        self.statement()?;

        let else_jump = self.chunk.append_chunk(OpCode::Jump(0), line);

        self.patch_jump(if_jump);

        if self.match_token(TokenType::Else) {
            self.statement()?;
        }

        self.patch_jump(else_jump);

        Ok(())
    }

    fn while_statement(&mut self) -> Result<(), CompilerError> {
        let loop_start = self.chunk.next();

        self.try_consume(TokenType::LeftParen, "Expected '(' after 'if'.")?;
        self.expression()?;
        let line = self
            .try_consume(TokenType::RightParen, "Expected ')' after condition.")?
            .line;

        let exit_jump = self.chunk.append_chunk(OpCode::JumpIfFalse(0), line);
        self.chunk.append_chunk(OpCode::Pop, line);

        self.statement()?;

        //Backwards offset instead of forward
        let offset = (self.chunk.top() + 2) - loop_start;
        self.chunk.append_chunk(OpCode::Loop(offset), line);

        self.patch_jump(exit_jump);

        self.chunk.append_chunk(OpCode::Pop, line);

        Ok(())
    }

    fn for_statement(&mut self) -> Result<(), CompilerError> {
        self.begin_scope(); //To capture the variable initializer

        self.try_consume(TokenType::LeftParen, "Expected '(' after 'for'.")?;
        if self.match_token(TokenType::Semicolon) {
            //No initializer
        } else if self.match_token(TokenType::Var) {
            self.var_declaration()?;
        } else {
            self.expression_statement()?;
        }

        let loop_start = self.chunk.next();

        let exit_jump = if !self.match_token(TokenType::Semicolon) {
            self.expression()?;
            let line = self
                .try_consume(TokenType::Semicolon, "Expected ';'.")?
                .line;

            let exit_jump = self.chunk.append_chunk(OpCode::JumpIfFalse(0), line);
            self.chunk.append_chunk(OpCode::Pop, line);
            Some(exit_jump)
        } else {
            None
        };

        let loop_start = if !self.match_token(TokenType::RightParen) {
            let line = self.peek().line;
            let body_jump = self.chunk.append_chunk(OpCode::Jump(0), line);

            let increment_start = self.chunk.next();

            self.expression()?;
            self.chunk.append_chunk(OpCode::Pop, line);
            self.try_consume(TokenType::RightParen, "Expected ')' after 'for' clauses.")?;

            let offset = (self.chunk.top() + 2) - loop_start;
            self.chunk.append_chunk(OpCode::Loop(offset), line);

            self.patch_jump(body_jump);
            increment_start
        } else {
            loop_start
        };

        self.statement()?;

        let line = self.peek().line;
        let offset = (self.chunk.top() + 2) - loop_start;
        self.chunk.append_chunk(OpCode::Loop(offset), line);

        if let Some(exit_jump) = exit_jump {
            self.patch_jump(exit_jump);
            self.chunk.append_chunk(OpCode::Pop, line);
        }

        self.end_scope();
        Ok(())
    }

    fn and(&mut self, _can_assign: bool) -> Result<(), CompilerError> {
        let line = self.peek().line;
        let end_jump = self.chunk.append_chunk(OpCode::JumpIfFalse(0), line);

        self.chunk.append_chunk(OpCode::Pop, line);

        self.parse_precedence(Precedence::And)?;

        self.patch_jump(end_jump);
        Ok(())
    }

    fn or(&mut self, _can_assign: bool) -> Result<(), CompilerError> {
        let line = self.peek().line;
        let else_jump = self.chunk.append_chunk(OpCode::JumpIfFalse(0), line);
        let end_jump = self.chunk.append_chunk(OpCode::Jump(0), line);

        self.patch_jump(else_jump);
        self.chunk.append_chunk(OpCode::Pop, line);

        self.parse_precedence(Precedence::Or)?;

        self.patch_jump(end_jump);
        Ok(())
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
