if condition
  top # comment 1
elsif :"#{a}"
      ^^^^^^^ Literal `:"#{a}"` appeared as a condition.
  foo # comment 2
else
  bar
end
