1. It is line 162. Here, the scanner makes the decision to consume the character '.' and the line 164 and 165 consume the number(s) after the dot.
For '5.', the integer loop consumes 5. But the 'peek_next()' returns nothing because there's nothing after the '.' character. So the scanner emits Number("5") and the '.' is left for the next scan_token call. This will report "Character is not part of any token". The scanner doesn't consume a '.' unless a digit follows it. 

2. 'self.line' changes in three places, and all three are 'self.line += 1'; 
Line 55: in the '\n' arm of the whitespace loop.
Line 79: in the block comment loop.
Line 142: in 'string()', at 'if self.peak() == '\n'.
Take a file that is x\n\n\n, meaning one token and then two blank lines. The scanner passes 3 newlines, so self.line is 4 when run() finishes. But run() computes eof_line = self.tokens.last().map_or(self.line, |t| t.line), so EOF carries line 1, the line of x. It only falls back to self.line when there are no tokens at all. I think section 6.1 wants EOF on the last token's line because trailing blank lines are just whitespace and shouldn't move the end-of-file position. Counting them would make EOF's line depend on how many blank lines happen to be at the end of the file.

3. I failed valid\eof_line.kobo, multiple times. It expected [line 1] EOF '' and I printed [line 9] as EOF '' for one of the error instances. So basically, it was passing the wrong value for the line number of the EOF function. I fixed it in run() by replacing self.add(TokenType::Eof) with the eof_line computation and the manual Token push. I had misunderstood which line EOF belongs to. I assumed it should carry whatever self.line was when scanning ended, so every trailing newline pushed it further down. The spec ties EOF to the last token, not to the scanner's position. The very last commit represented the fixed code, whereas the one before that had 4 out of 14 tests passed.

The git hash where I only passed about 4 of the tests is this: c54c315c9f0174b1567f061e0be1a2d3afe8ed79
I had just written the identifier function so I had run the tests for the first time after all my functions were written and I only passed 4 out of the 14 tests.

The git hash where I pass all my tests and I corrected the EOF not passing errors is this: f94be1141d46a7e795c5f1d479cbd4adfa0e5560