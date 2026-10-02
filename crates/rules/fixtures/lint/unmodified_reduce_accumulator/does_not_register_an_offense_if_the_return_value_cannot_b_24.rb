enum.inject do |acc, el|
  x = [*acc, el]
  x << 42 if foo
  x
end
