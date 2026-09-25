def for(param)
  ret = 1

  for x in param...10
    param += x
    ret = param + 1
  end

  ret
end
