enum.reduce do |acc, el|
  x = [*acc, el]
  x << 42 if foo
  x
end
