test {
  foo(<<~FIRST).bar(<<~SECOND) }
                               ^ Expression at 2, 32 should be on its own line.
    Heredoc text.
  FIRST
    Heredoc text.
  SECOND
