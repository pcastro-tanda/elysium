while x > 0
  redo if x.odd?

  case x
  when 1
    break
  else
    raise MyError
  end
end
