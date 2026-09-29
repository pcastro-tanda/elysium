def foo?
  return !bar.do_something.nil? if condition
  baz
  !bar.nil?
end
