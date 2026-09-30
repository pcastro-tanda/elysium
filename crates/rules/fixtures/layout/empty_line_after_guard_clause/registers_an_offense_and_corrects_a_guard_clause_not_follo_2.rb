def method
  if truthy
    raise <<-MSG
      This is an error.
    MSG
  end
  ^^^ Add empty line after guard clause.
  value
end
