def foo?
  bar
  !baz.do_something.nil?
rescue
  qux
else
  quux
ensure
  corge
end
