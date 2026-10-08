def arr
  [1, 2, 3]
end

arr.reverse.each { |e| puts e }
    ^^^^^^^^^^^^ Use `reverse_each` instead of `reverse.each`.
