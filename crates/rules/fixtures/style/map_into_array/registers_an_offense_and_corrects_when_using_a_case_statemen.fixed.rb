dest = src.map do |e|
  case foo
  when 1
    e
  else
    e * 2
  end
end
