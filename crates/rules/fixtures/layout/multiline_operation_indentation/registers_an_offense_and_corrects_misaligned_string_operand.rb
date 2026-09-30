def f
  flash[:error] = 'Here is a string ' \
                  'That spans' <<
      'multiple lines'
      ^^^^^^^^^^^^^^^^ Align the operands of an expression in an assignment spanning multiple lines.
end
