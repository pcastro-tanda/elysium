def do_something(bar)
  bar = 'baz' if foo
  bar ||= {}
end
