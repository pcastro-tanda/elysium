def some_method(flag_a, flag_b)
  foo = 1

  if flag_a
    puts foo
    2
  else
    case
    when flag_b
      puts foo
    end
  end
end
