def do_something(*items)
  *items, last = items
  puts items
end
