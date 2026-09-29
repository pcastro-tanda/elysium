r = [1, 2, 3].reduce({}) do |memo, item|
  memo += item > 2 ? item : 0
  memo
end
