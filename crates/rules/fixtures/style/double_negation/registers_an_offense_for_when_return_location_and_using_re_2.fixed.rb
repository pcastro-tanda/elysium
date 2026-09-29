def foo?
  bar
  !baz.do_something.nil?
rescue
  baz
ensure
  qux
end
