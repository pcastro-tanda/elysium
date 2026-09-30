def f
  flash[:error] = 'Here is a string ' \
                  'That spans' <<
                  'multiple lines'
end
