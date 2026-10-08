values.inject do |acc, el|
  el << acc.foo
  el
end
