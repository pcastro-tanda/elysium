def foo
  def fail; end
end

fail
bar
^^^ Unreachable code detected.
