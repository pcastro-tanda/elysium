if foo?
^^ Use a guard clause (`unless foo?; raise 'very long and detailed description of the error that makes it too long to fit on one line'; end`) instead of wrapping the code inside a conditional expression.
else
  raise 'very long and detailed description of the error that makes it too long to fit on one line'
end
