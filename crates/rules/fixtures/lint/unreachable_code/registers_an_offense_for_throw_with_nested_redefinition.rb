def foo
  def throw; end
end

throw
bar
^^^ Unreachable code detected.
