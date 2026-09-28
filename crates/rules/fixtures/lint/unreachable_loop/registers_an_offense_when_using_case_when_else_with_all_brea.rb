while x > 0
^^^^^^^^^^^ This loop will have at most one iteration.
  case x
  when 1
    break
  else
    raise MyError
  end
end
