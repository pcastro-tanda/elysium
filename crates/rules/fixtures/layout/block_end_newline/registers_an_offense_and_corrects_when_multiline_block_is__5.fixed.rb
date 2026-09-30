test {
  foo(<<~FIRST, <<~SECOND)
    Heredoc text.
  FIRST
    Heredoc text.
  SECOND
}
