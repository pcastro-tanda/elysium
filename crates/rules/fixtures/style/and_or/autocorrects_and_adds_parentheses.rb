items.each do |item|
  foo and next 1
      ^^^ Use `&&` instead of `and`.
  bar and break 2
      ^^^ Use `&&` instead of `and`.
end
