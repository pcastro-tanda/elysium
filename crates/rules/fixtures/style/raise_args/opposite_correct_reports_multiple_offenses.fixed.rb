if a
  raise RuntimeError, msg
elsif b
  raise Ex, msg
else
  raise ArgumentError, msg
end
