foo.each do |v|
  if v
    v&.bar
     ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
  end
end
