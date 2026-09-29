def do_something(foo, bar)
  bar.do_something == bar || foo == :sym
end
