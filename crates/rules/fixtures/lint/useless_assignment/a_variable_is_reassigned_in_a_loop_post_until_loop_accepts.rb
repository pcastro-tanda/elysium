def post_until(param)
  ret = 1

  begin
    param += 2
    ret = param + 1
  end until param == 10

  ret
end
