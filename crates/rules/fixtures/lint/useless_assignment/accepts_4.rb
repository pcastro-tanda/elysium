def some_method
  if baz
    foo = 1
  else
    foo = 2
    bar {
      foo = 3
    }
  end

  foo
end
