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

        let eof_line = self.tokens.last().map_or(self.line, |t| t.line);
        self.tokens.push(Token {
            kind: TokenType::Eof,
            lexeme: String::new(),
            line: eof_line,
        });
    }

    fn scan_token(&mut self) {
        // Skip whitespace and comments before identifying a token.
        loop {
            if self.at_end() {
                return;
            }

            match self.peek() {
                ' ' | '\r' | '\t' => {
                    self.advance();
                }

                '\n' => {
                    self.line += 1;
                    self.advance();
                }

                '/' if self.peek_next() == '/' => {
                    while !self.at_end() && self.peek() != '\n' {
                        self.advance();
                    }
                }

                '/' if self.peek_next() == '*' => {
                    let start_line = self.line;
                    self.advance(); // '/'
                    self.advance(); // '*'

                    let mut terminated = false;
                    while !self.at_end() {
                        if self.peek() == '*' && self.peek_next() == '/' {
                            self.advance();
                            self.advance();
                            terminated = true;
                            break;
                        }
                        if self.peek() == '\n' {
                            self.line += 1;
                        }
                        self.advance();
                    }

                    if !terminated {
                        self.error(start_line, "Unterminated block comment.");
                        return;
                    }
                }

                _ => break,
            }
        }

        // start is now the first character of the actual token.
        self.start = self.current;

        let c = self.advance();

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
            '/' => self.add(TokenType::Slash),

            '!' => {
                let kind = if self.matches('=') { TokenType::BangEqual } else { TokenType::Bang };
                self.add(kind);
            }
            '=' => {
                let kind = if self.matches('=') { TokenType::EqualEqual } else { TokenType::Equal };
                self.add(kind);
            }
            '<' => {
                let kind = if self.matches('=') { TokenType::LessEqual } else { TokenType::Less };
                self.add(kind);
            }
            '>' => {
                let kind = if self.matches('=') { TokenType::GreaterEqual } else { TokenType::Greater };
                self.add(kind);
            }

            '"' => self.string(),
            '0'..='9' => self.number(),
            'a'..='z' | 'A'..='Z' | '_' => self.identifier(),

            _ => self.error(self.line, "Character is not part of any token."),
        }
    }

    fn string(&mut self) {
        let start_line = self.line; // report errors at where the string began

        while !self.at_end() && self.peek() != '"' {
            if self.peek() == '\n' {
                self.line += 1;
            }
            self.advance();
        }

        if self.at_end() {
            self.error(start_line, "String is never closed.");
            return;
        }

        self.advance(); // closing quote
        self.add(TokenType::Str);
    }

    fn number(&mut self) {
        while !self.at_end() && self.peek().is_ascii_digit() {
            self.advance();
        }

        // Fractional part: '.' must be followed by a digit
        if !self.at_end() && self.peek() == '.' && self.peek_next().is_ascii_digit() {
            self.advance(); // '.'
            while !self.at_end() && self.peek().is_ascii_digit() {
                self.advance();
            }
        }

        self.add(TokenType::Number);
    }

    fn identifier(&mut self) {
        while !self.at_end() && (self.peek().is_ascii_alphanumeric() || self.peek() == '_') {
            self.advance();
        }

        let text: String = self.src[self.start..self.current].iter().collect();

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