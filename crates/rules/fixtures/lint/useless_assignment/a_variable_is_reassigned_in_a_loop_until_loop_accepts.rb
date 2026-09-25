def until(param)
  ret = 1

  until param == 10
    param += 2
    ret = param + 1
  end

  ret
end
