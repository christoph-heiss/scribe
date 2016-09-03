use buffer::{Lexeme, Position, Token};
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxDefinition};
use buffer::token::line_iterator::LineIterator;
use std::vec::IntoIter;

pub struct TokenIterator<'a> {
    scopes: ScopeStack,
    parser: ParseState,
    line_tokens: Option<IntoIter<Token<'a>>>,
    lines: LineIterator<'a>
}

impl<'a> TokenIterator<'a> {
    pub fn new(data: &'a str, def: &SyntaxDefinition) -> TokenIterator<'a> {
        TokenIterator{
            scopes: ScopeStack::new(),
            parser: ParseState::new(def),
            line_tokens: None,
            lines: LineIterator::new(data)
        }
    }

    fn next_token(&mut self) -> Option<Token<'a>> {
        // Try to fetch a token from the current line.
        if let Some(ref mut tokens) = self.line_tokens {
            if let Some(token) = tokens.next() {
                return Some(token)
            }
        }

        // We're done with this line; on to the next.
        self.parse_next_line();

        // If this returns none, we're done.
        if let Some(ref mut tokens) = self.line_tokens {
            tokens.next()
        } else {
            None
        }
    }

    fn parse_next_line(&mut self) {
        let mut tokens = Vec::new();
        let mut offset = 0;

        if let Some((line_number, line)) = self.lines.next() {
            if line_number > 0 {
                // We've found another line, so push a newline token.
                tokens.push(Token::Newline);
            }

            for (change_offset, scope_change) in self.parser.parse_line(line) {
                // We only want to capture the deepest scope for a given token,
                // so we apply all of them and only capture once we move on to
                // another token/offset.
                if change_offset > offset {
                    tokens.push(
                        Token::Lexeme(Lexeme{
                            value: &line[offset..change_offset],
                            scope: self.scopes.as_slice().last().map(|s| s.clone()),
                            position: Position{
                                line: line_number,
                                offset: offset
                            }
                        })
                    );
                    offset = change_offset;
                }

                // Apply the scope and keep a reference to it, so
                // that we can pair it with a token later on.
                self.scopes.apply(&scope_change);

            }

            // We already have discrete variant for newlines,
            // so exclude them when considering content length.
            let line_length = line_length(line);
            if offset < line_length {
                // The rest of the line hasn't triggered a scope
                // change; categorize it with the last known scope.
                tokens.push(
                    Token::Lexeme(Lexeme{
                        value: &line[offset..line_length],
                        scope: self.scopes.as_slice().last().map(|s| s.clone()),
                        position: Position{
                            line: line_number,
                            offset: offset
                        }
                    })
                );
            }

            self.line_tokens = Some(tokens.into_iter());
        } else {
            self.line_tokens = None;
        }
    }
}

fn line_length(line: &str) -> usize {
    if line.chars().last() == Some('\n') {
        line.len() - 1
    } else {
        line.len()
    }
}

impl<'a> Iterator for TokenIterator<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(token) = self.next_token() {
            return Some(token)
        }

        self.parse_next_line();
        self.next_token()
    }
}

#[cfg(test)]
mod tests {
    use super::TokenIterator;
    use buffer::{Lexeme, Position, Scope, Token};
    use syntect::parsing::SyntaxSet;

    #[test]
    fn token_iterator_returns_correct_tokens() {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let def = syntax_set.find_syntax_by_extension("rs");
        let iterator = TokenIterator::new("struct Buffer {\n  data: String\n}garbage\n\n", def.unwrap());
        let expected_tokens = vec![
            Token::Lexeme(Lexeme{
                value: "struct",
                scope: Some(Scope::new("storage.type.struct.rust").unwrap()),
                position: Position{ line: 0, offset: 0 }
            }),
            Token::Lexeme(Lexeme{
                value: " ",
                scope: Some(Scope::new("meta.struct.rust").unwrap()),
                position: Position{ line: 0, offset: 6 }
            }),
            Token::Lexeme(Lexeme{
                value: "Buffer",
                scope: Some(Scope::new("entity.name.struct.rust").unwrap()),
                position: Position{ line: 0, offset: 7 }
            }),
            Token::Lexeme(Lexeme{
                value: " ",
                scope: Some(Scope::new("meta.struct.rust").unwrap()),
                position: Position{ line: 0, offset: 13 }
            }),
            Token::Lexeme(Lexeme{
                value: "{",
                scope: Some(Scope::new("punctuation.definition.block.begin.rust").unwrap()),
                position: Position{ line: 0, offset: 14 }
            }),
            Token::Newline,
            Token::Lexeme(Lexeme{
                value: "  ",
                scope: Some(Scope::new("meta.block.rust").unwrap()),
                position: Position{ line: 1, offset: 0 }
            }),
            Token::Lexeme(Lexeme{
                value: "data",
                scope: Some(Scope::new("variable.other.property.rust").unwrap()),
                position: Position{ line: 1, offset: 2 }
            }),
            Token::Lexeme(Lexeme{
                value: ":",
                scope: Some(Scope::new("punctuation.separator.rust").unwrap()),
                position: Position{ line: 1, offset: 6 }
            }),
            Token::Lexeme(Lexeme{
                value: " String",
                scope: Some(Scope::new("meta.block.rust").unwrap()),
                position: Position{ line: 1, offset: 7 }
            }),
            Token::Newline,
            Token::Lexeme(Lexeme{
                value: "}",
                scope: Some(Scope::new("punctuation.definition.block.end.rust").unwrap()),
                position: Position{ line: 2, offset: 0 }
            }),
            Token::Lexeme(Lexeme{
                value: "garbage",
                scope: Some(Scope::new("source.rust").unwrap()),
                position: Position{ line: 2, offset: 1 }
            }),
            Token::Newline,
            Token::Newline
        ];
        let actual_tokens: Vec<Token> = iterator.collect();
        println!("{:?}", actual_tokens);
        for (index, token) in expected_tokens.into_iter().enumerate() {
            assert_eq!(token, actual_tokens[index]);
        }

        //assert_eq!(expected_tokens, actual_tokens);
    }

    #[test]
    fn token_iterator_handles_content_without_trailing_newline() {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let def = syntax_set.find_syntax_by_extension("rs");
        let iterator = TokenIterator::new("struct", def.unwrap());
        let expected_tokens = vec![
            Token::Lexeme(Lexeme{
                value: "struct",
                scope: Some(Scope::new("storage.type.struct.rust").unwrap()),
                position: Position{ line: 0, offset: 0 }
            })
        ];
        let actual_tokens: Vec<Token> = iterator.collect();
        for (index, token) in expected_tokens.into_iter().enumerate() {
            assert_eq!(token, actual_tokens[index]);
        }

        //assert_eq!(expected_tokens, actual_tokens);
    }
}
