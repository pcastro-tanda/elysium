def foo
  if bar > baz
  ^^ Remove redundant `if` with boolean literal branches.
    true
  else
    false
  end
end
