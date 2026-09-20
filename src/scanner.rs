use std::io::ErrorKind;

use crate::token::{keyword, Token, TokenType};

/// Cut `source` into tokens. Returns everything it managed to scan alongside every
/// error it found; the caller decides whether to go on.
pub fn scan(source: &str) -> (Vec<Token>, Vec<String>) {
    let mut s = Scanner {
        src: source.chars().collect(),
        start: 0,
        current: 0,
        line: 1,
        tokens: Vec::new(),
        errors: Vec::new(),
    };
    s.run();
    (s.tokens, s.errors)
}

struct Scanner {
    src: Vec<char>,
    start: usize,
    current: usize,
    line: usize,
    tokens: Vec<Token>,
    errors: Vec<String>,
}

impl Scanner {
    fn run(&mut self) {
        while !self.at_end() {
            self.start = self.current; 
            self.scan_token();         
        }
        self.add(TokenType::Eof);
    }

    fn scan_token(&mut self) {
    // Skip whitespace and comments
    loop {
        match self.advance() {
            ' ' | '\r' | '\t' => {}, // Ignore whitespace
            '\n' => self.line += 1,  // Newline increases count by 1
            '/' => {
                if self.matches('/') {
                    // Line comment: skip to end of line
                    while !self.at_end() && self.peek() != '\n' { self.advance(); }
                } else if self.matches('*') {
                    // Block comment: skip until closing */
                    while !self.at_end() {
                        if self.peek() == '*' && self.peek_next() == '/' {
                            self.advance(); // consume '*'
                            self.advance(); // consume '/'
                            break;
                        }
                        if self.peek() == '\n' { self.line += 1; }
                        self.advance();
                    }
                } else {
                    // Single '/' is division
                    self.add(TokenType::Slash);
                    return;
                }
            }
            c => {
                // Non-whitespace, non-slash: re-consume as token start
                self.current -= 1;
                break;
            }
        }
    }

    if self.at_end() { return; } // Guard against EOF after skipping

    self.start = self.current; // Mark start of token
    let c = self.advance();    // Consume first char
    match c {
        '(' => self.add(TokenType::LParen),
        ')' => self.add(TokenType::RParen),
        '{' => self.add(TokenType::LBrace),
        '}' => self.add(TokenType::RBrace),
        ',' => self.add(TokenType::Comma),
        ';' => self.add(TokenType::Semicolon),
        '+' => self.add(TokenType::Plus),
        '-' => self.add(TokenType::Minus),
        '*' => self.add(TokenType::Star),
        '!' => {
            if self.matches('=') { self.add(TokenType::BangEqual); }
            else { self.add(TokenType::Bang); }
        }
        '=' => {
            if self.matches('=') { self.add(TokenType::EqualEqual); }
            else { self.add(TokenType::Equal); }
        }
        '<' => {
            if self.matches('=') { self.add(TokenType::LessEqual); }
            else { self.add(TokenType::Less); }
        }
        '>' => {
            if self.matches('=') { self.add(TokenType::GreaterEqual); }
            else { self.add(TokenType::Greater); }
        }
        '"' => self.string(), // String literal
        '0'..='9' => self.number(), // Number literal
        'a'..='z' | 'A'..='Z' | '_' => self.identifier(), // Identifier or keyword
        _ => self.error(self.line, "Character is not part of any token."), // Error
    }
}


    fn string(&mut self) {
        // Consume character until closing quote
        while !self.at_end() && self.peek() != '"' {
            if self.peek() == '\n' { self.line += 1; } // Newline in string
            self.advance();
        }

        if self.at_end() {
            // Unterminated string literal
            self.error(self.line, "Unterminated string.");
            return;
        }

        // Consume the closing quote
        self.advance();

        // Add the string token
        self.add(TokenType::Str);
    }

    fn number(&mut self) {
        // Integer part
        while !self.at_end() && self.peek().is_ascii_digit() {
            self.advance();
        }

        // Fractional part
        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            self.advance(); // Consume '.'
            while !self.at_end() && self.peek().is_ascii_digit() {
                self.advance(); // Consume digits after decimal point
            }
        }

        self.add(TokenType::Number);
    }

    fn identifier(&mut self) {
        // Consume all identifier characters (letters, digits, underscores)
        while !self.at_end() && (self.peek().is_ascii_alphanumeric() || self.peek() == '_') {
            self.advance();
    }

    // Extract the lexeme as string
    let text: String = self.src[self.start..self.current].iter().collect();

    // Check if it's a keyword
    if let Some(kind) = keyword(&text) {
        self.add(kind);
    } else {
        self.add(TokenType::Identifier);
    }
}

    // --- primitives ---------------------------------------------------------------

    fn at_end(&self) -> bool {
        self.current >= self.src.len()
    }

    fn advance(&mut self) -> char {
        let c = self.src[self.current];
        self.current += 1;
        c
    }

    fn matches(&mut self, expected: char) -> bool {
        if self.at_end() || self.src[self.current] != expected {
            return false;
        }
        self.current += 1;
        true
    }

    fn peek(&self) -> char {
        if self.at_end() {
            '\0'
        } else {
            self.src[self.current]
        }
    }

    fn peek_next(&self) -> char {
        if self.current + 1 >= self.src.len() {
            '\0'
        } else {
            self.src[self.current + 1]
        }
    }

    fn add(&mut self, kind: TokenType) {
        self.tokens.push(Token {
            kind,
            lexeme: self.src[self.start..self.current].iter().collect(),
            line: self.line,
        });
    }

    fn error(&mut self, line: usize, message: &str) {
        self.errors.push(format!("[line {}] Error: {}", line, message));
    }
}
