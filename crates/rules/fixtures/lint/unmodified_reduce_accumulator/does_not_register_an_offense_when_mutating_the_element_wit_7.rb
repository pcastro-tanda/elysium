values.reduce do |acc, el|
  el &&= acc
  el
end
