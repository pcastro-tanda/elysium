while x > 0
  case x
  when 1
    break
  when 2
    do_something
  else
    raise MyError
  end
end
