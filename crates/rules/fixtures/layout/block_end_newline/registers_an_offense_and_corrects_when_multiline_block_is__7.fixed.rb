test {
  foo(<<~FIRST).bar(<<~SECOND)
    Heredoc text.
  FIRST
    Heredoc text.
  SECOND
}
