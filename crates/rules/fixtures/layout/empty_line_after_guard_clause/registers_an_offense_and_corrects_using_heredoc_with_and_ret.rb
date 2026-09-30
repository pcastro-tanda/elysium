def foo
  puts(<<~MSG) and return if bar
    A multiline
    message
  MSG
^^^^^ Add empty line after guard clause.
  baz
end
