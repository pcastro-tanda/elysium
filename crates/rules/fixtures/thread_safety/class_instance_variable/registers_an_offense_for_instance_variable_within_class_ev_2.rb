def separate_with(separator)
  ::Example.class_eval do
    @separator = separator
    ^^^^^^^^^^ Avoid class instance variables.
  end
end
