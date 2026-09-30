def func
  raise <<~MESSAGE.strip unless condition
      oops
    MESSAGE
foo
end
