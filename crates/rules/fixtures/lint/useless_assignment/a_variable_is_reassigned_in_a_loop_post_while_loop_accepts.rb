def post_while(param)
  ret = 1

  begin
    param += 2
    ret = param + 1
  end while param < 40

  ret
end
