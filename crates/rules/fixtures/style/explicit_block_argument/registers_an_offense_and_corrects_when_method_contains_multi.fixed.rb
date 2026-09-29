def m(&block)
  items.something(&block)

  if condition
    yield 2
  elsif other_condition
    3.times(&block)
  else
    other_items.something(&block)
  end
end
