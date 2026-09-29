if a
  raise RuntimeError.new(msg)
elsif b
  raise Ex.new(msg)
else
  raise ArgumentError.new(msg)
end
