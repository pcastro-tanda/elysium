def foo
  raise if <<~TEXT.length > bar
    hi
  TEXT

  baz
end
