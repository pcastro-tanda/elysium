values.reduce do |acc, el|
  el &&= acc.foo
  el
end
