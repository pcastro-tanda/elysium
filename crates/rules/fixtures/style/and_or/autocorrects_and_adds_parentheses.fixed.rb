items.each do |item|
  foo && (next 1)
  bar && (break 2)
end
