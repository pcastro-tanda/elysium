def method
  if truthy
    raise <<-MSG
      This is an error.
    MSG
  end

  value
end
