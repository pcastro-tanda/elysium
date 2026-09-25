def some_method
  case condition
  when foo then
    foo = 1
  else
    bar.each do |foo|
    end
  end
end
