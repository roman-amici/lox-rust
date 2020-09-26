use super::token::Token;
use super::token::TokenType;

#[derive(Debug)]
pub struct ScannerError {
    pub line: usize,
    pub description: String,
}

struct LexicalScanner {
    //State variables
    start: usize,
    current: usize,
    line: usize,

    //Data Variables
    source_chars: Vec<char>,
    tokens: Vec<Token>,
}

impl LexicalScanner {
    pub fn new(source: &String) -> LexicalScanner {
        LexicalScanner {
            start: 0,
            current: 0,
            line: 1,
            source_chars: source.chars().collect(),
            tokens: vec![],
        }
    }

    pub fn consume_token(&mut self, token_type: TokenType, literal: Option<String>) {
        let t = Token {
            token_type,
            lexeme: self.make_lexeme(),
            line: self.line,
            literal,
        };
        self.tokens.push(t);

        self.start = self.current;
    }

    pub fn make_lexeme(&self) -> String {
        // The easiest way to convert to a string from a char array seems to be to
        // convert to a vector and then collect the resulting iterator...
        let s: String = self.source_chars[self.start..self.current]
            .to_vec()
            .into_iter()
            .collect();
        s
    }

    pub fn make_literal(&self, start: usize, end: usize) -> String {
        let s: String = self.source_chars[start..end].to_vec().into_iter().collect();
        s
    }

    pub fn match_ahead(&mut self, char_to_match: char) -> bool {
        if self.current < self.source_chars.len() {
            let c = self.source_chars[self.current];
            if c == char_to_match {
                self.current += 1;
                return true;
            }
        }
        false
    }

    pub fn has_next(&self) -> bool {
        self.current < self.source_chars.len()
    }

    pub fn next(&mut self) -> char {
        let c = self.source_chars[self.current];
        self.current += 1;
        c
    }

    pub fn peek(&self) -> char {
        if self.current < self.source_chars.len() {
            self.source_chars[self.current]
        } else {
            '\0'
        }
    }

    pub fn peek_next(&self) -> char {
        if self.current + 1 < self.source_chars.len() {
            self.source_chars[self.current + 1]
        } else {
            '\0'
        }
    }

    pub fn peek_prev(&self) -> char {
        if self.current != 0 {
            self.source_chars[self.current - 1]
        } else {
            '\0'
        }
    }

    pub fn consume_whitespace(&mut self) {
        self.start = self.current;
    }

    pub fn consume_comment(&mut self) {
        while self.has_next() {
            let c = self.next();
            if c == '\n' {
                self.line += 1;
                break;
            }
        }

        self.start = self.current;
    }

    pub fn consume_string(&mut self) -> Result<(), ScannerError> {
        while self.has_next() {
            match self.next() {
                '\n' => {
                    self.line += 1;
                }
                '\"' => {
                    let literal = self.make_literal(self.start + 1, self.current - 1);
                    self.consume_token(TokenType::StringToken, Some(literal));
                    return Ok(());
                }
                _ => {}
            }
        }

        Err(ScannerError {
            line: self.line,
            description: String::from("Unterminated string!"),
        })
    }

    pub fn consume_number(&mut self) -> Result<(), ScannerError> {
        while self.has_next() {
            if LexicalScanner::is_digit(self.peek()) {
                self.next();
            } else {
                break;
            }
        }

        if self.peek() == '.' && LexicalScanner::is_digit(self.peek_next()) {
            self.next();
            while self.has_next() {
                if LexicalScanner::is_digit(self.peek()) {
                    self.next();
                } else {
                    break;
                }
            }
        }

        let literal = self.make_literal(self.start, self.current);
        self.consume_token(TokenType::NumberToken, Some(literal));

        Ok(())
    }

    pub fn consume_identifier_or_keyword(&mut self) -> Result<(), ScannerError> {
        while self.has_next() {
            match self.peek() {
                '0'..='9' | 'a'..='z' | 'A'..='Z' => {
                    self.next();
                }
                _ => {
                    break;
                }
            }
        }

        let literal = self.make_literal(self.start, self.current);
        let token_type = LexicalScanner::literal_to_token_type(&literal);

        self.consume_token(token_type, Some(literal));

        Ok(())
    }

    pub fn is_digit(digit: char) -> bool {
        match digit {
            '0'..='9' => true,
            _ => false,
        }
    }

    pub fn literal_to_token_type(literal: &String) -> TokenType {
        //Todo, figure out a way to use a hashtable.
        match &literal[..] {
            "and" => TokenType::And,
            "class" => TokenType::Class,
            "else" => TokenType::Else,
            "false" => TokenType::False,
            "true" => TokenType::True,
            "fun" => TokenType::Fun,
            "for" => TokenType::For,
            "if" => TokenType::If,
            "nil" => TokenType::Nil,
            "or" => TokenType::Or,
            "print" => TokenType::Print,
            "return" => TokenType::Return,
            "super" => TokenType::Super,
            "this" => TokenType::This,
            "var" => TokenType::Var,
            "while" => TokenType::While,
            "EOF" => TokenType::EOF,
            _ => TokenType::Identifier,
        }
    }
}

pub fn scan_tokens(source: &String) -> Result<Vec<Token>, ScannerError> {
    let mut scanner = LexicalScanner::new(source);

    while scanner.has_next() {
        let c = scanner.next();

        let token_or_error = match c {
            '(' => Ok(scanner.consume_token(TokenType::LeftParen, None)),
            ')' => Ok(scanner.consume_token(TokenType::RightParen, None)),
            '{' => Ok(scanner.consume_token(TokenType::LeftBrace, None)),
            '}' => Ok(scanner.consume_token(TokenType::RightBrace, None)),
            ',' => Ok(scanner.consume_token(TokenType::Comma, None)),
            '.' => Ok(scanner.consume_token(TokenType::Dot, None)),
            '-' => Ok(scanner.consume_token(TokenType::Minus, None)),
            '+' => Ok(scanner.consume_token(TokenType::Plus, None)),
            ';' => Ok(scanner.consume_token(TokenType::Semicolon, None)),
            '*' => Ok(scanner.consume_token(TokenType::Star, None)),
            '!' => {
                let token_type = if scanner.match_ahead('=') {
                    TokenType::BangEqual
                } else {
                    TokenType::Bang
                };
                Ok(scanner.consume_token(token_type, None))
            }
            '=' => {
                let token_type = if scanner.match_ahead('=') {
                    TokenType::EqualEqual
                } else {
                    TokenType::Equal
                };
                Ok(scanner.consume_token(token_type, None))
            }
            '<' => {
                let token_type = if scanner.match_ahead('=') {
                    TokenType::LessEqual
                } else {
                    TokenType::Less
                };
                Ok(scanner.consume_token(token_type, None))
            }
            '>' => {
                let token_type = if scanner.match_ahead('=') {
                    TokenType::GreaterEqual
                } else {
                    TokenType::Greater
                };
                Ok(scanner.consume_token(token_type, None))
            }
            '/' => {
                if scanner.match_ahead('/') {
                    scanner.consume_comment();
                    Ok(())
                } else {
                    Ok(scanner.consume_token(TokenType::Slash, None))
                }
            }
            ' ' => {
                scanner.consume_whitespace();
                Ok(())
            }
            '\r' => {
                scanner.consume_whitespace();
                Ok(())
            }
            '\t' => {
                scanner.consume_whitespace();
                Ok(())
            }
            '\n' => {
                scanner.consume_whitespace();
                scanner.line += 1;
                Ok(())
            }
            '\"' => scanner.consume_string(),
            '0'..='9' => scanner.consume_number(),
            'a'..='z' | 'A'..='Z' => scanner.consume_identifier_or_keyword(),
            _ => Err(ScannerError {
                line: scanner.line,
                description: String::from("Unrecognized Token"),
            }),
        };

        token_or_error?;
    }

    scanner.tokens.push(Token {
        token_type: TokenType::EOF,
        lexeme: String::from("EOF"),
        line: 0,
        literal: None,
    });
    Ok(scanner.tokens)
}

#[cfg(test)]
mod scanner_tests {
    use super::*;

    #[test]
    fn single_tokens() {
        let test_input = String::from("() \r { \n  +-;");
        let tokens = scan_tokens(&test_input).unwrap();

        assert_eq!(tokens.len(), 6);
        assert_eq!(tokens[0].token_type, TokenType::LeftParen);
        assert_eq!(tokens[1].token_type, TokenType::RightParen);
        assert_eq!(tokens[2].token_type, TokenType::LeftBrace);
        assert_eq!(tokens[3].token_type, TokenType::Plus);
        assert_eq!(tokens[4].token_type, TokenType::Minus);
        assert_eq!(tokens[5].token_type, TokenType::Semicolon);
    }

    #[test]
    fn multi_equals() {
        let test_input = String::from("== = <=\n< >= > !==");
        let tokens = scan_tokens(&test_input).unwrap();

        assert_eq!(tokens.len(), 8);
        assert_eq!(tokens[0].token_type, TokenType::EqualEqual);
        assert_eq!(tokens[1].token_type, TokenType::Equal);
        assert_eq!(tokens[2].token_type, TokenType::LessEqual);
        assert_eq!(tokens[3].token_type, TokenType::Less);
        assert_eq!(tokens[4].token_type, TokenType::GreaterEqual);
        assert_eq!(tokens[5].token_type, TokenType::Greater);
        assert_eq!(tokens[6].token_type, TokenType::BangEqual);
        assert_eq!(tokens[7].token_type, TokenType::Equal);
    }

    #[test]
    fn slash_comments() {
        let test_input = String::from("// This is to be filtered out\n / ");
        let tokens = scan_tokens(&test_input).unwrap();

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::Slash);
    }

    #[test]
    fn regular_string() {
        let test_input = String::from(" \"this is a test string\"");
        let tokens = scan_tokens(&test_input).unwrap();

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::StringToken);
        let s = tokens[0].literal.as_ref().unwrap();
        assert_eq!(*s, String::from("this is a test string"));
    }

    #[test]
    fn multiline_string() {
        let test_input = String::from("\n \" This is a \n 3multiline \n string\" \n");
        let tokens = scan_tokens(&test_input).unwrap();

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token_type, TokenType::StringToken);
        let s = tokens[0].literal.as_ref().unwrap();
        assert_eq!(*s, String::from(" This is a \n 3multiline \n string"));
    }

    #[test]
    fn number_token() {
        let test_input = String::from("343 893.4 \n");
        let tokens = scan_tokens(&test_input).unwrap();

        println!("{:?}", tokens);
        assert_eq!(tokens.len(), 2);

        assert_eq!(tokens[0].token_type, TokenType::NumberToken);
        let n1: f64 = tokens[0].literal.as_ref().unwrap().parse().unwrap();
        assert_eq!(n1, 343.0);

        assert_eq!(tokens[1].token_type, TokenType::NumberToken);
        let n2: f64 = tokens[1].literal.as_ref().unwrap().parse().unwrap();
        assert_eq!(n2, 893.4);
    }

    #[test]
    fn identifier_token() {
        let test_input = String::from(" valid one123 123one");
        let tokens = scan_tokens(&test_input).unwrap();

        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[1].token_type, TokenType::Identifier);
        assert_eq!(tokens[2].token_type, TokenType::NumberToken);
        assert_eq!(tokens[3].token_type, TokenType::Identifier);

        assert_eq!(*tokens[0].literal.as_ref().unwrap(), String::from("valid"));
        assert_eq!(*tokens[1].literal.as_ref().unwrap(), String::from("one123"));
        assert_eq!(*tokens[2].literal.as_ref().unwrap(), String::from("123"));
        assert_eq!(*tokens[3].literal.as_ref().unwrap(), String::from("one"));
    }

    #[test]
    fn keyword_token() {
        let test_input = String::from(" valid class! classical true this;");
        let tokens = scan_tokens(&test_input).unwrap();

        assert_eq!(tokens.len(), 7);
        assert_eq!(tokens[0].token_type, TokenType::Identifier);
        assert_eq!(tokens[1].token_type, TokenType::Class);
        assert_eq!(tokens[3].token_type, TokenType::Identifier);
        assert_eq!(tokens[4].token_type, TokenType::True);
        assert_eq!(tokens[5].token_type, TokenType::This);

        assert_eq!(*tokens[0].literal.as_ref().unwrap(), String::from("valid"));
        assert_eq!(
            *tokens[3].literal.as_ref().unwrap(),
            String::from("classical")
        );
    }
}
