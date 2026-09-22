if something
^^ Use a guard clause (`raise "error" unless something`) instead of wrapping the code inside a conditional expression.
 puts "hello"
else
  raise "error"
end
