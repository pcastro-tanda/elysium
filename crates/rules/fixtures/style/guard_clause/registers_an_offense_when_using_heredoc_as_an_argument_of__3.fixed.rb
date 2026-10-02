def func
  raise <<~MESSAGE unless condition
      oops
    MESSAGE
end
