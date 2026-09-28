while x > 0
  case x
  in 1
    break
  in 2
    do_something
  else
    raise MyError
  end
end
