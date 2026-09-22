def func
  return unless condition
    raise <<~MESSAGE
      oops
    MESSAGE
end
