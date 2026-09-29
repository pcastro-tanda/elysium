def do_something(foo)
  foo = bar
  ^^^^^^^^^ Argument `foo` was shadowed by a local variable before it was used.
  begin
  rescue
    foo = baz
  end
  puts foo
end
