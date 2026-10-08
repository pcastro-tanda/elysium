def arr
  [1, 2]
end

arr.
  reverse.
  ^^^^^^^^ Use `reverse_each` instead of `reverse.each`.
  each { |e| puts e }
