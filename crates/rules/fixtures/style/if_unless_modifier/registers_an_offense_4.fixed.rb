def f
  if items.filter_map { |item| item.do_something if item.something? }
    return value
  end
end
