if condition
^^ Use a guard clause (`raise e unless condition`) instead of wrapping the code inside a conditional expression.
  do_something
else
  raise e
end
