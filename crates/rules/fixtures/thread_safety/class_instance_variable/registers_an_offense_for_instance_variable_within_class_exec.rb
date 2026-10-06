def separate_with(separator)
  Example.class_exec do
    @separator = separator
    ^^^^^^^^^^ Avoid class instance variables.
  end
end
