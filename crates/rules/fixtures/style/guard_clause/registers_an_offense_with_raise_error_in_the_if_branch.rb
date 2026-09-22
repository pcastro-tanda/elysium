if something
^^ Use a guard clause (`raise "error" if something`) instead of wrapping the code inside a conditional expression.
  raise "error"
else
  puts "hello"
end
