def foo
  return true if <<~TEXT.length > bar
    hi
  TEXT

  false
end
