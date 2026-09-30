test {
  foo(<<~FIRST, <<~SECOND) }
                           ^ Expression at 2, 28 should be on its own line.
    Heredoc text.
  FIRST
    Heredoc text.
  SECOND
