if condition
  arr.select { |x| x > 0 }
else
  arr.reject { |x| x > 0 }
end
