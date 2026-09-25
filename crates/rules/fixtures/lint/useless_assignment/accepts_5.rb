def some_method
  case baz
  when 1
    foo = 1
  else
    foo = 2
    bar {
      foo = 3
    }
  end

  foo
end
