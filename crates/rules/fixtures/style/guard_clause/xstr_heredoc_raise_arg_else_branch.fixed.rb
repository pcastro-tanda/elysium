def func
  raise <<~`MESSAGE` unless condition
      oops
    MESSAGE
foo
end
